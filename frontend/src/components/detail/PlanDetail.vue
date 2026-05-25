<template>
  <div class="plan-detail">
    <div v-for="(phase, idx) in phases" :key="idx" class="phase-card">
      <div class="phase-header">
        <div class="phase-title">
          <span class="phase-num">P{{ phase.number }}</span>
          <span class="phase-name">{{ phase.title }}</span>
          <span v-if="phase.version" class="phase-version">{{ phase.version }}</span>
        </div>
        <div class="phase-progress">
          <div class="progress-bar">
            <div class="progress-fill" :style="{ width: phase.progress + '%' }" />
          </div>
          <span class="progress-text">{{ phase.completed }}/{{ phase.tasks.length }}</span>
        </div>
      </div>
      <ul class="task-list">
        <li
          v-for="(task, tidx) in phase.tasks"
          :key="tidx"
          :class="{ done: task.done }"
        >
          <span class="task-check">{{ task.done ? '✓' : '○' }}</span>
          <div class="task-body">
            <button
              class="task-title-btn"
              :class="{ active: activeTaskId === task.id }"
              @click="toggleTask(task.id)"
            >
              {{ task.title }}
            </button>
            <div v-if="activeTaskId === task.id" class="task-detail-popover">
              <div class="task-detail-content">
                <div v-if="task.detail" class="task-detail-text">{{ task.detail }}</div>
                <div v-else class="task-detail-empty">No detail provided.</div>
                <div class="task-detail-meta">
                  <span v-if="task.owner" class="task-meta-item">👤 {{ task.owner }}</span>
                  <span v-if="task.duration" class="task-meta-item">⏱️ {{ task.duration }}</span>
                  <span v-if="task.dependencies" class="task-meta-item">🔗 {{ task.dependencies }}</span>
                  <span class="task-meta-item" :class="task.status.toLowerCase().replace(/\s+/g, '_')">{{ task.status }}</span>
                </div>
              </div>
            </div>
          </div>
        </li>
      </ul>
    </div>
    <MarkdownContent v-if="remainingContent" :content="remainingContent" @link-click="$emit('linkClick', $event)" />
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import MarkdownContent from '@/components/MarkdownContent.vue'

const props = defineProps<{
  content: string
}>()

const emit = defineEmits<{
  linkClick: [id: string]
}>()

const activeTaskId = ref<string | null>(null)

function toggleTask(id: string) {
  activeTaskId.value = activeTaskId.value === id ? null : id
}

interface Task {
  id: string
  title: string
  detail: string
  owner: string
  duration: string
  dependencies: string
  status: string
  done: boolean
}

interface Phase {
  number: number
  title: string
  version: string
  tasks: Task[]
  completed: number
  progress: number
}

function parseMarkdownTable(lines: string[], startIdx: number): { rows: string[][], endIdx: number } {
  const rows: string[][] = []
  let i = startIdx
  // Header row
  const headerLine = lines[i]?.trim()
  if (!headerLine || !headerLine.startsWith('|')) return { rows, endIdx: startIdx }
  rows.push(splitTableRow(headerLine))
  i++
  // Separator row
  if (i < lines.length && lines[i]?.trim().startsWith('|')) {
    i++ // skip separator like |---|---|---|
  }
  // Data rows
  while (i < lines.length) {
    const line = lines[i]?.trim()
    if (!line || !line.startsWith('|')) break
    rows.push(splitTableRow(line))
    i++
  }
  return { rows, endIdx: i }
}

function splitTableRow(line: string): string[] {
  return line
    .split('|')
    .map(c => c.trim())
    .filter((c, idx, arr) => {
      // Keep non-empty cells, but also keep empty cells between pipes
      if (idx === 0 || idx === arr.length - 1) return c !== ''
      return true
    })
}

const phases = computed<Phase[]>(() => {
  const result: Phase[] = []
  const lines = props.content.split('\n')

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i].trimEnd()

    // Match plan heading like "## P1 Agents Relay — Foundation" or "## P1: Title"
    const planHeadingMatch = line.match(/^##\s+P(\d+)(?::|\s+)(.+)/i)
    if (planHeadingMatch) {
      // We don't create phases from plan headings — the whole content is one plan
      continue
    }

    // Match phase heading like "## Phase 1: Foundation (v0.1)"
    const phaseHeadingMatch = line.match(/^##\s+Phase\s+(\d+):\s+(.+?)(?:\s+\(([^)]+)\))?\s*$/i)
    if (phaseHeadingMatch) {
      const phase: Phase = {
        number: parseInt(phaseHeadingMatch[1]),
        title: phaseHeadingMatch[2].trim(),
        version: phaseHeadingMatch[3] || '',
        tasks: [],
        completed: 0,
        progress: 0,
      }
      result.push(phase)
      continue
    }

    // Match table rows
    if (line.trim().startsWith('|')) {
      const { rows, endIdx } = parseMarkdownTable(lines, i)
      i = endIdx - 1

      if (rows.length >= 2) {
        // Find column indices
        const header = rows[0].map(h => h.toLowerCase().trim())
        const taskIdx = header.findIndex(h => h.includes('task'))
        const detailIdx = header.findIndex(h => h.includes('detail'))
        const ownerIdx = header.findIndex(h => h.includes('owner'))
        const durationIdx = header.findIndex(h => h.includes('duration'))
        const depsIdx = header.findIndex(h => h.includes('depend') || h.includes('deps'))
        const statusIdx = header.findIndex(h => h.includes('status'))
        const phaseIdx = header.findIndex(h => h.includes('phase'))

        // Determine which phase to add tasks to
        let targetPhase = result[result.length - 1]
        if (!targetPhase) {
          targetPhase = {
            number: 0,
            title: 'Tasks',
            version: '',
            tasks: [],
            completed: 0,
            progress: 0,
          }
          result.push(targetPhase)
        }

        for (let r = 1; r < rows.length; r++) {
          const row = rows[r]
          if (row.every(c => c === '' || c.match(/^-+$/))) continue // skip separator

          const status = statusIdx >= 0 ? row[statusIdx] || '' : ''
          const task: Task = {
            id: `${targetPhase.number}-${r}`,
            title: taskIdx >= 0 ? row[taskIdx] || '' : '',
            detail: detailIdx >= 0 ? row[detailIdx] || '' : '',
            owner: ownerIdx >= 0 ? row[ownerIdx] || '' : '',
            duration: durationIdx >= 0 ? row[durationIdx] || '' : '',
            dependencies: depsIdx >= 0 ? row[depsIdx] || '' : '',
            status,
            done: /done|complete|implemented/i.test(status),
          }
          targetPhase.tasks.push(task)
          if (task.done) targetPhase.completed++
        }

        targetPhase.progress = targetPhase.tasks.length
          ? Math.round((targetPhase.completed / targetPhase.tasks.length) * 100)
          : 0
      }
      continue
    }

    // Legacy checkbox format: "- [x] Task text"
    const checkboxMatch = line.match(/^-\s+\[([ xX])\]\s+(.+)$/)
    if (checkboxMatch) {
      const targetPhase = result[result.length - 1]
      if (targetPhase) {
        const done = checkboxMatch[1].toLowerCase() === 'x'
        targetPhase.tasks.push({
          id: `${targetPhase.number}-${targetPhase.tasks.length}`,
          title: checkboxMatch[2].trim(),
          detail: '',
          owner: '',
          duration: '',
          dependencies: '',
          status: done ? 'Done' : '',
          done,
        })
        if (done) targetPhase.completed++
        targetPhase.progress = targetPhase.tasks.length
          ? Math.round((targetPhase.completed / targetPhase.tasks.length) * 100)
          : 0
      }
    }
  }

  return result
})

const remainingContent = computed(() => {
  const lines = props.content.split('\n')
  const kept: string[] = []
  let inTable = false

  for (const raw of lines) {
    const line = raw.trimEnd()

    if (line.match(/^##\s+Phase\s+\d+/i)) {
      continue
    }

    if (line.trim().startsWith('|')) {
      inTable = true
      continue
    }

    if (inTable && !line.trim().startsWith('|')) {
      inTable = false
    }

    if (line.match(/^-\s+\[[ xX]\]/)) {
      continue
    }

    if (line.trim()) kept.push(line)
  }

  return kept.join('\n')
})
</script>

<style scoped>
.plan-detail {
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.phase-card {
  border: 1px solid var(--af-border);
  border-radius: 10px;
  padding: 0.9rem 1rem;
  background: hsl(var(--muted-foreground) / 0.02);
}

.phase-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.6rem;
}

.phase-title {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.phase-num {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  border-radius: 6px;
  background: hsl(var(--primary) / 0.1);
  color: hsl(var(--primary));
  font-size: 0.78rem;
  font-weight: 700;
}

.phase-name {
  font-size: 0.98rem;
  font-weight: 600;
  color: var(--af-fg);
}

.phase-version {
  font-size: 0.78rem;
  color: var(--af-muted);
  background: hsl(var(--muted-foreground) / 0.08);
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
}

.phase-progress {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.progress-bar {
  width: 80px;
  height: 6px;
  border-radius: 3px;
  background: hsl(var(--muted-foreground) / 0.1);
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  border-radius: 3px;
  background: hsl(var(--primary));
  transition: width 0.3s ease;
}

.progress-text {
  font-size: 0.78rem;
  color: var(--af-muted);
  font-variant-numeric: tabular-nums;
}

.task-list {
  list-style: none;
  padding: 0;
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.task-list li {
  display: flex;
  align-items: flex-start;
  gap: 0.4rem;
  font-size: 0.93rem;
  color: var(--af-fg);
  line-height: 1.4;
}

.task-list li.done {
  opacity: 0.55;
}

.task-list li.done .task-title-btn {
  text-decoration: line-through;
  color: var(--af-muted);
}

.task-check {
  font-size: 0.83rem;
  color: hsl(var(--primary));
  min-width: 1rem;
  text-align: center;
  margin-top: 0.15rem;
}

.task-list li.done .task-check {
  color: hsl(142 71% 45%);
}

.task-body {
  flex: 1;
  min-width: 0;
}

.task-title-btn {
  background: none;
  border: none;
  padding: 0;
  margin: 0;
  font-size: 0.93rem;
  color: hsl(var(--primary));
  cursor: pointer;
  text-align: left;
  text-decoration: underline;
  text-decoration-color: hsl(var(--primary) / 0.3);
  text-underline-offset: 2px;
  transition: all 0.15s;
  line-height: 1.4;
}

.task-title-btn:hover {
  text-decoration-color: hsl(var(--primary));
  color: hsl(var(--primary) / 0.85);
}

.task-title-btn.active {
  color: hsl(var(--primary));
  text-decoration-color: hsl(var(--primary));
  font-weight: 500;
}

.task-detail-popover {
  margin-top: 0.4rem;
  margin-bottom: 0.4rem;
  padding: 0.75rem 1rem;
  background: hsl(var(--muted-foreground) / 0.04);
  border: 1px solid hsl(var(--primary) / 0.15);
  border-radius: 8px;
  animation: detailFadeIn 0.15s ease;
}

@keyframes detailFadeIn {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}

.task-detail-text {
  font-size: 0.88rem;
  line-height: 1.6;
  color: var(--af-fg);
  white-space: pre-wrap;
}

.task-detail-empty {
  font-size: 0.88rem;
  color: var(--af-muted);
  font-style: italic;
}

.task-detail-meta {
  display: flex;
  flex-wrap: wrap;
  gap: 0.4rem;
  margin-top: 0.6rem;
  padding-top: 0.5rem;
  border-top: 1px solid var(--af-border);
}

.task-meta-item {
  font-size: 0.78rem;
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
  background: hsl(var(--muted-foreground) / 0.06);
  color: var(--af-muted);
}

.task-meta-item.done,
.task-meta-item.complete,
.task-meta-item.implemented {
  background: hsl(142 71% 45% / 0.1);
  color: hsl(142 71% 45%);
}

.task-meta-item.draft,
.task-meta-item.proposed {
  background: hsl(38 92% 50% / 0.1);
  color: hsl(38 92% 50%);
}
</style>
