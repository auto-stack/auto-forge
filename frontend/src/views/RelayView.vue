<template>
  <div class="agents-view" data-testid="relay-view">
    <div class="agents-header">
      <h2>{{ t('relay.title') }}</h2>
      <div class="agents-actions">
        <button class="btn-secondary" @click="refresh">
          <RefreshCw :size="14" />
        </button>
        <button
          v-if="finishedRunCount > 0"
          class="btn-secondary btn-danger-outline"
          @click="onDeleteFinishedRuns"
          :disabled="loading"
          :title="t('relay.deleteAllTitle', { count: finishedRunCount })"
        >
          <Trash2 :size="14" />
          {{ t('relay.clearFinished', { count: finishedRunCount }) }}
        </button>
      </div>
    </div>

    <div v-if="error" class="error-banner">{{ error }}</div>

    <div class="agents-body">
      <!-- Left: Runs list -->
      <div class="runs-sidebar" data-testid="relay-run-list">
        <div class="panel-title">Runs</div>
        <div v-if="runs.length === 0" class="empty-state">No runs yet</div>
        <div
          v-for="run in runs"
          :key="run.run_id"
          class="run-card"
          :class="{ active: currentRun?.run_id === run.run_id }"
          :data-testid="`run-card-${run.run_id}`"
          :title="run.task ? run.task.slice(0, 50) : ''"
          @click="selectRun(run.run_id)"
        >
          <div class="run-card-header">
            <div class="run-card-title-col">
              <span class="run-id" :title="run.run_id">{{ run.title || run.run_id }}</span>
              <span class="run-id-sub">{{ run.run_id }}</span>
            </div>
            <StatusBadge :status="run.status" />
          </div>
          <div class="run-card-meta">
            <span>{{ run.current_profession ?? '—' }}</span>
            <span>{{ formatTokens(run.cumulative_tokens) }}</span>
          </div>
          <div class="run-progress-bar">
            <div
              class="run-progress-fill"
              :style="{ width: runProgressPercent(run) + '%' }"
            />
          </div>
          <button
            class="btn-icon btn-delete"
            :title="t('relay.deleteRun')"
            @click.stop="onDeleteRun(run.run_id)"
          >
            <Trash2 :size="12" />
          </button>
        </div>
      </div>

      <!-- Center: Pipeline visualization -->
      <div class="pipeline-panel" data-testid="pipeline-panel">
        <div v-if="!currentRun" class="empty-state">
          {{ t('relay.selectRun') }}
        </div>

        <template v-else>
          <!-- Run header -->
          <div class="run-header">
            <div class="run-title">
              {{ currentRun.title || currentRun.run_id }}
              <span v-if="currentRun.title" class="run-title-id">{{ currentRun.run_id }}</span>
            </div>
            <div class="run-stats">
              <span class="stat-badge">
                <Coins :size="12" />
                {{ formatTokens(currentRun.cumulative_tokens) }}
              </span>
              <span class="stat-badge">
                <Zap :size="12" />
                {{ t('relay.saved', { percent: Math.round(currentRun.savings_ratio * 100) }) }}
              </span>
            </div>
          </div>

          <!-- Budget bar -->
          <div class="budget-bar-container">
            <div class="budget-label">
              <span>Budget</span>
              <span>{{ formatTokens(currentRun.budget_limit - currentRun.budget_remaining) }} / {{ formatTokens(currentRun.budget_limit) }}</span>
            </div>
            <div class="budget-bar">
              <div
                class="budget-fill"
                :class="{ warning: budgetUsedPercent > 70, danger: budgetUsedPercent > 90 }"
                :style="{ width: budgetUsedPercent + '%' }"
              />
            </div>
          </div>

          <!-- Pipeline steps -->
          <div class="pipeline-flow" data-testid="pipeline-flow">
            <template v-for="(step, idx) in currentRun.steps" :key="step.id">
              <div
                class="pipeline-step"
                :class="[step.status, { expanded: expandedStepId === step.id }]"
                :title="`${step.profession_id} (${step.gate})`"
                data-testid="pipeline-step"
                :aria-label="`${step.profession_id} step, status ${step.status}${step.gate === 'human' ? ', human gate required' : ''}`"
                role="button"
                tabindex="0"
                @click="toggleStep(step.id)"
                @keydown.enter="toggleStep(step.id)"
              >
                <AgentAvatar :profession-id="step.profession_id" size="sm" aria-hidden="true" />
                <div class="step-name">{{ step.profession_id }}</div>
                <div v-if="step.gate === 'human'" class="step-gate" aria-hidden="true">🔒</div>
                <div v-if="step.status === 'running'" class="step-pulse" aria-hidden="true" />
                <div v-if="stepIteration(step.id) > 1" class="step-retry" aria-label="Retry iteration {{ stepIteration(step.id) }}">
                  ×{{ stepIteration(step.id) }}
                </div>
              </div>

              <!-- Expanded node card -->
              <div
                v-if="expandedStepId === step.id"
                class="expanded-step-card"
              >
                <div class="expanded-header">
                  <AgentAvatar :profession-id="step.profession_id" size="md" />
                  <span class="expanded-name">{{ step.profession_id }}</span>
                  <StatusBadge :status="step.status" size="sm" />
                </div>
                <div class="expanded-metrics">
                  <div class="metric">
                    <span class="metric-label">{{ t('relay.gate') }}</span>
                    <span class="metric-value">{{ step.gate }}</span>
                  </div>
                  <div class="metric">
                    <span class="metric-label">{{ t('relay.iterations') }}</span>
                    <span class="metric-value">{{ stepIteration(step.id) }}</span>
                  </div>
                  <div class="metric">
                    <span class="metric-label">{{ t('relay.tokens') }}</span>
                    <span class="metric-value">{{ formatTokens(stepTokens(step.id)) }}</span>
                  </div>
                </div>
                <div class="expanded-actions">
                  <span class="expanded-hint">{{ t('relay.clickToCollapse') }}</span>
                </div>
              </div>

              <div v-if="idx < currentRun.steps.length - 1" class="step-connector">
                <ChevronRight :size="14" />
              </div>
            </template>
          </div>

          <!-- Session Log with Step Timeline Navigator -->
          <div class="session-log-panel">
            <div class="panel-title">Session Log</div>
            <div class="session-log-body">
              <!-- Left: Log entries -->
              <div ref="sessionLogRef" class="session-log-main">
                <div v-if="sessionLog.length === 0" class="empty-state">
                  Start a run to see agent activity
                </div>
                <div v-else class="session-log-list">
                  <div
                    v-for="entry in sessionLog"
                    :key="entry.id"
                    :data-step-id="entry.step_id || ''"
                    class="session-entry"
                    :class="`type-${entry.type}`"
                  >
                    <div class="session-entry-header">
                      <AgentAvatar :profession-id="entry.profession_id" size="xs" />
                      <span class="session-profession">{{ entry.profession_id }}</span>
                      <span class="session-time">{{ entry.time }}</span>
                    </div>

                    <!-- Text content -->
                    <div v-if="entry.type === 'text'" class="session-text">
                      <pre>{{ entry.content }}</pre>
                    </div>

                    <!-- Unified Tool Widget -->
                    <div v-else-if="entry.type === 'tool'" class="session-tool unified">
                      <div class="tool-header">
                        <Wrench :size="12" />
                        <span>{{ entry.tool_name }}</span>
                        <span class="tool-badge">tool</span>
                      </div>
                      <div class="tool-body">
                        <details class="tool-details">
                          <summary>Arguments</summary>
                          <pre>{{ JSON.stringify(entry.arguments, null, 2) }}</pre>
                        </details>
                        <details class="tool-details result">
                          <summary>Result</summary>
                          <pre>{{ entry.result }}</pre>
                        </details>
                      </div>
                    </div>

                    <!-- Tool call (orphan, no matching result yet) -->
                    <div v-else-if="entry.type === 'tool_call'" class="session-tool">
                      <div class="tool-header">
                        <Wrench :size="12" />
                        <span>{{ entry.tool_name }}</span>
                        <span class="tool-badge pending">pending</span>
                      </div>
                      <details class="tool-details">
                        <summary>Arguments</summary>
                        <pre>{{ JSON.stringify(entry.arguments, null, 2) }}</pre>
                      </details>
                    </div>

                    <!-- Tool result (orphan, no matching call) -->
                    <div v-else-if="entry.type === 'tool_result'" class="session-tool result">
                      <div class="tool-header">
                        <CheckCircle :size="12" />
                        <span>Result</span>
                      </div>
                      <details class="tool-details">
                        <summary>Output</summary>
                        <pre>{{ entry.content }}</pre>
                      </details>
                    </div>

                    <!-- Complete -->
                    <div v-else-if="entry.type === 'complete'" class="session-complete">
                      <CheckCircle :size="12" />
                      <span>Turn completed</span>
                    </div>

                    <!-- Error -->
                    <div v-else-if="entry.type === 'error'" class="session-error">
                      <AlertCircle :size="12" />
                      <span>{{ entry.content }}</span>
                    </div>

                    <!-- Budget warning -->
                    <div v-else-if="entry.type === 'budget_warning'" class="session-warning">
                      <AlertTriangle :size="12" />
                      <span>{{ entry.content }}</span>
                    </div>

                    <!-- Budget exceeded -->
                    <div v-else-if="entry.type === 'budget_exceeded'" class="session-error">
                      <AlertCircle :size="12" />
                      <span>{{ entry.content }}</span>
                    </div>

                    <!-- Step started -->
                    <div v-else-if="entry.type === 'step_started'" :id="`step-${entry.step_id}`" class="session-step">
                      <Play :size="12" />
                      <span>{{ entry.content }}</span>
                    </div>

                    <!-- Step completed -->
                    <div v-else-if="entry.type === 'step_completed'" class="session-step completed">
                      <CheckCircle :size="12" />
                      <span>{{ entry.content }}</span>
                    </div>

                    <!-- Gate waiting -->
                    <div v-else-if="entry.type === 'gate_waiting'" class="session-warning">
                      <AlertTriangle :size="12" />
                      <span>{{ entry.content }}</span>
                    </div>

                    <!-- Run completed -->
                    <div v-else-if="entry.type === 'run_completed'" class="session-step completed">
                      <CheckCircle :size="12" />
                      <span>{{ entry.content }}</span>
                    </div>

                    <!-- Run failed -->
                    <div v-else-if="entry.type === 'run_failed'" class="session-error">
                      <AlertCircle :size="12" />
                      <span>{{ entry.content }}</span>
                    </div>
                  </div>
                </div>
              </div>

              <!-- Right: Vertical step timeline -->
              <div v-if="stepTimeline.length > 0" class="step-timeline">
                <div
                  v-for="(record, idx) in stepTimeline"
                  :key="record.step_id + record.started_at + record.iteration"
                  class="step-timeline-item"
                  :class="{ active: activeStepNav === idx }"
                  @click="scrollToStep(idx)"
                >
                  <div class="step-timeline-dot" />
                  <div class="step-timeline-content">
                    <span class="step-timeline-agent">
                      <AgentAvatar :profession-id="record.profession_id" size="xs" />
                      <span>{{ record.profession_id }} x{{ record.iteration + 1 }}</span>
                    </span>
                    <span class="step-timeline-time">{{ formatTime(record.started_at) }}</span>
                  </div>
                </div>
              </div>
            </div>
          </div>

          <!-- Cost Breakdown with Pie Chart -->
          <div class="cost-panel">
            <div class="panel-title">Cost Breakdown</div>
            <div v-if="!currentRun" class="empty-state">—</div>
            <div v-else class="cost-chart">
              <div v-if="pieSlices.length > 0" class="pie-chart-container">
                <svg viewBox="0 0 100 100" class="pie-chart">
                  <g transform="translate(50,50) rotate(-90)">
                    <circle
                      v-for="(slice, i) in pieSlices"
                      :key="i"
                      r="25"
                      fill="transparent"
                      :stroke="slice.color"
                      :stroke-width="slice.profession === currentRun?.current_profession ? 22 : 18"
                      :stroke-dasharray="`${slice.length} ${slice.gap}`"
                      :stroke-dashoffset="slice.offset"
                      :class="{ 'pie-slice-active': slice.profession === currentRun?.current_profession }"
                    />
                  </g>
                  <g transform="translate(50,50)">
                    <text y="1" text-anchor="middle" dominant-baseline="middle" class="pie-total">
                      {{ formatTokens(currentRun.cumulative_tokens) }}
                    </text>
                  </g>
                </svg>
              </div>
              <div v-else class="empty-state" style="flex:1;text-align:left;padding:0;">
                Per-profession data will appear as steps run
              </div>
              <div v-if="pieSlices.length > 0" class="pie-legend">
                <div
                  v-for="(slice, i) in pieSlices"
                  :key="i"
                  class="pie-legend-item"
                  :class="{ active: slice.profession === currentRun?.current_profession }"
                >
                  <span class="pie-dot" :style="{ background: slice.color }" />
                  <span class="pie-prof">{{ slice.profession }}</span>
                  <span class="pie-val">{{ formatTokens(slice.tokens) }}</span>
                </div>
              </div>
            </div>
          </div>

          <!-- Gate approval panel -->
          <GatePanel
            v-if="showGatePanel && currentRun.waiting_for_gate"
            :run-id="currentRun.run_id"
            :gate="currentGate!"
            :profession-id="currentRun.waiting_for_gate.profession_id"
            @approve="onApprove"
            @reject="onReject"
            @review-in-specs="onReviewInSpecs"
          />
        </template>
      </div>

    </div>

  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import {
  Play, RefreshCw, Coins, Zap, ChevronRight,
  Trash2, Wrench, CheckCircle, AlertCircle, AlertTriangle,
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useRelay } from '@/composables/useRelay'
import { useGateInbox } from '@/composables/useGateInbox'
import { useForgeMode } from '@/composables/useForgeMode'
import StatusBadge from '@/components/StatusBadge.vue'
import GatePanel from '@/components/GatePanel.vue'
import AgentAvatar from '@/components/AgentAvatar.vue'

const {
  runs, currentRun, professions, souls, loading, error,
  hasActiveGate, budgetUsedPercent, liveLog, professionTokens, sessionLog,
  loadProfessions, loadSouls, loadRuns, loadRun,
  resolveGate, subscribeToRun, deleteRun,
} = useRelay()

const { t } = useI18n()

const gateInbox = useGateInbox()
const { shouldPauseGate } = useForgeMode()

const expandedStepId = ref<string | null>(null)
const sessionLogRef = ref<HTMLElement | null>(null)
const activeStepNav = ref<number>(-1)

// Auto-scroll session log to bottom when new entries arrive
import { watch, nextTick } from 'vue'
watch(() => sessionLog.value.length, async () => {
  await nextTick()
  if (sessionLogRef.value) {
    sessionLogRef.value.scrollTop = sessionLogRef.value.scrollHeight
  }
})

// Profession color palette for pie chart
const professionColors: Record<string, string> = {
  assistant: 'hsl(220 70% 60%)',
  advisor: 'hsl(280 60% 60%)',
  planner: 'hsl(200 80% 55%)',
  architect: 'hsl(160 60% 45%)',
  coder: 'hsl(35 90% 55%)',
  tester: 'hsl(50 80% 50%)',
  reviewer: 'hsl(340 70% 55%)',
  documenter: 'hsl(260 55% 60%)',
  gofer: 'hsl(180 50% 50%)',
  manager: 'hsl(30 60% 50%)',
}

const pieSlices = computed(() => {
  if (!currentRun.value) return []
  const entries = Object.entries(professionTokens.value)
  if (entries.length === 0) return []
  const total = currentRun.value.cumulative_tokens || 1
  const circumference = 2 * Math.PI * 25 // r=25
  let accumulated = 0
  return entries.map(([profession, tokens]) => {
    const ratio = tokens / total
    const length = ratio * circumference
    const gap = circumference - length
    const offset = -accumulated // start from top
    accumulated += length
    return {
      profession,
      tokens,
      color: professionColors[profession] ?? `hsl(${profession.split('').reduce((a, c) => a + c.charCodeAt(0), 0) % 360} 60% 55%)`,
      length,
      gap,
      offset,
    }
  })
})

const stepTimeline = computed(() => {
  if (!currentRun.value) return []
  const items = [...currentRun.value.step_history]

  // If a step is currently running, append an in-progress record
  if (currentRun.value.status === 'running' && currentRun.value.current_step_started_at) {
    const currentStep = currentRun.value.steps[currentRun.value.current_step]
    if (currentStep) {
      const iteration = items.filter(h => h.step_id === currentStep.id).length
      items.push({
        step_id: currentStep.id,
        profession_id: currentStep.profession_id,
        started_at: currentRun.value.current_step_started_at,
        completed_at: 0,
        iteration,
      })
    }
  }

  return items
})

function scrollToStep(timelineIndex: number) {
  activeStepNav.value = timelineIndex
  const record = stepTimeline.value[timelineIndex]
  if (!record || !sessionLogRef.value) return

  const entries = sessionLogRef.value.querySelectorAll('.session-entry.type-step_started')
  let matchCount = 0
  for (let i = 0; i < entries.length; i++) {
    const el = entries[i]
    if (el.getAttribute('data-step-id') === record.step_id) {
      if (matchCount === record.iteration) {
        sessionLogRef.value.scrollTo({
          top: (el as HTMLElement).offsetTop - 10,
          behavior: 'smooth',
        })
        return
      }
      matchCount++
    }
  }
}

function toggleStep(stepId: string) {
  expandedStepId.value = expandedStepId.value === stepId ? null : stepId
}

function stepIteration(stepId: string): number {
  if (!currentRun.value) return 0
  return currentRun.value.step_history.filter((h) => h.step_id === stepId).length
}

function stepTokens(stepId: string): number {
  // Derive from profession tokens if available, else estimate from history
  const step = currentRun.value?.steps.find((s) => s.id === stepId)
  if (!step) return 0
  return professionTokens.value[step.profession_id] || 0
}

const showGatePanel = computed(() => {
  if (!hasActiveGate.value || !currentRun.value?.waiting_for_gate) return false
  // In GSD mode, only show gates that are goal-level
  return shouldPauseGate(currentRun.value.waiting_for_gate.profession_id)
})

const currentGate = computed(() => {
  if (!currentRun.value?.waiting_for_gate) return null
  return {
    gateId: `${currentRun.value.run_id}-${currentRun.value.waiting_for_gate.step_id}`,
    runId: currentRun.value.run_id,
    profession: currentRun.value.waiting_for_gate.profession_id,
    title: `${currentRun.value.waiting_for_gate.profession_id} needs approval`,
    since: currentRun.value.waiting_for_gate.since,
    status: 'pending' as const,
  }
})

let unsubscribe: (() => void) | null = null

onMounted(async () => {
  await loadProfessions()
  await loadSouls()
  await loadRuns()
})

onUnmounted(() => {
  if (unsubscribe) unsubscribe()
})

function selectRun(runId: string) {
  if (unsubscribe) unsubscribe()
  sessionLog.value = []
  loadRun(runId)
  unsubscribe = subscribeToRun(runId)
}

async function refresh() {
  await loadRuns()
  if (currentRun.value) {
    await loadRun(currentRun.value.run_id)
  }
}

async function onDeleteRun(runId: string) {
  if (confirm('Delete this run?')) {
    await deleteRun(runId)
  }
}

const finishedRunCount = computed(() =>
  runs.value.filter((r: any) => r.status === 'completed' || r.status === 'failed').length,
)

async function onDeleteFinishedRuns() {
  const finishedRuns = runs.value.filter((r: any) => r.status === 'completed' || r.status === 'failed')
  if (finishedRuns.length === 0) return
  if (!confirm(`Delete ${finishedRuns.length} completed/failed run(s)?`)) return
  for (const run of finishedRuns) {
    await deleteRun(run.run_id)
  }
}

async function onApprove(runId: string) {
  await resolveGate(runId, 'approve')
}

async function onReject(runId: string) {
  await resolveGate(runId, 'reject', 'Needs revision')
}

function onReviewInSpecs(sectionId: string) {
  alert(`Navigate to specs section: ${sectionId}`)
}

function formatTokens(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return `${n}`
}

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function runProgressPercent(run: { current_step: number; total_steps: number }): number {
  if (run.total_steps === 0) return 0
  return Math.round((run.current_step / run.total_steps) * 100)
}

function professionIcon(id: string): string {
  const map: Record<string, string> = {
    assistant: '📥', advisor: '💡', planner: '📝', architect: '🏗️',
    coder: '💻', tester: '🧪', reviewer: '🔍', documenter: '📚',
  }
  return map[id] ?? '⚙️'
}
</script>

<style scoped>
.agents-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.agents-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  height: 48px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--af-border);
  gap: 1rem;
}

.agents-header h2 {
  font-size: 0.83rem;
  font-weight: 500;
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  line-height: 1;
  margin: 0;
}

.agents-actions {
  display: flex;
  gap: 0.5rem;
}

.btn-primary, .btn-secondary, .btn-approve, .btn-reject, .btn-edit, .btn-add, .btn-icon {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.4rem 0.7rem;
  border-radius: 6px;
  border: none;
  font-size: 0.83rem;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-primary {
  background: var(--af-primary);
  color: white;
}

.btn-primary:hover:not(:disabled) {
  opacity: 0.9;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary, .btn-icon {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-fg);
}

.btn-secondary:hover, .btn-icon:hover {
  background: hsl(var(--muted-foreground) / 0.14);
}

.btn-approve { background: hsl(142 70% 45% / 0.15); color: hsl(142 70% 35%); }
.btn-reject { background: hsl(0 70% 45% / 0.15); color: hsl(0 70% 45%); }
.btn-edit { background: hsl(220 70% 50% / 0.15); color: hsl(220 70% 45%); }
.btn-add { background: transparent; color: var(--af-muted); border: 1px dashed var(--af-border); width: 100%; justify-content: center; }
.btn-danger-outline { background: hsl(0 70% 45% / 0.1); color: hsl(0 70% 45%); border-color: hsl(0 70% 45% / 0.3); }
.btn-danger-outline:hover { background: hsl(0 70% 45% / 0.2); }

.error-banner {
  padding: 0.5rem 1.25rem;
  background: hsl(0 70% 50% / 0.08);
  color: hsl(0 70% 45%);
  font-size: 0.88rem;
  border-bottom: 1px solid var(--af-border);
}

.agents-body {
  flex: 1;
  display: grid;
  grid-template-columns: 220px 1fr 180px;
  gap: 1px;
  background: var(--af-border);
  overflow: hidden;
}

.runs-sidebar, .pipeline-panel {
  background: var(--af-bg);
  overflow-y: auto;
  padding: 0.75rem;
}

.panel-title {
  font-size: 0.78rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--af-muted);
  margin-bottom: 0.5rem;
}

.empty-state {
  font-size: 0.88rem;
  color: var(--af-muted);
  text-align: center;
  padding: 1rem 0;
}

/* Run cards */
.run-card {
  position: relative;
  padding: 0.6rem;
  padding-bottom: 1.6rem;
  border-radius: 6px;
  border: 1px solid var(--af-border);
  margin-bottom: 0.5rem;
  cursor: pointer;
  transition: all 0.15s;
}

.run-card:hover, .run-card.active {
  border-color: hsl(var(--primary) / 0.3);
  background: hsl(var(--primary) / 0.03);
}

.run-card-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 0.5rem;
  margin-bottom: 0.3rem;
}

.btn-delete {
  position: absolute;
  bottom: 0.4rem;
  right: 0.5rem;
  color: hsl(0 70% 50%);
  opacity: 0;
  transition: opacity 0.15s;
  z-index: 2;
}

.run-card:hover .btn-delete {
  opacity: 1;
}

.run-card-title-col {
  display: flex;
  flex-direction: column;
  min-width: 0;
  flex: 1;
}

.run-id {
  font-size: 0.83rem;
  font-weight: 500;
  color: var(--af-fg);
  font-family: 'JetBrains Mono', monospace;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.run-id-sub {
  font-size: 0.72rem;
  color: var(--af-muted);
  font-family: 'JetBrains Mono', monospace;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.run-card-meta {
  display: flex;
  justify-content: space-between;
  font-size: 0.78rem;
  color: var(--af-muted);
  margin-bottom: 0.4rem;
}

.run-progress-bar {
  height: 4px;
  background: hsl(var(--muted-foreground) / 0.08);
  border-radius: 2px;
  overflow: hidden;
}

.run-progress-fill {
  height: 100%;
  background: var(--af-primary);
  border-radius: 2px;
  transition: width 0.3s ease;
}

/* Pipeline */
.run-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
}

.run-title {
  font-size: 0.93rem;
  font-weight: 600;
  color: var(--af-fg);
  font-family: 'JetBrains Mono', monospace;
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.run-title-id {
  font-size: 0.78rem;
  font-weight: 400;
  color: var(--af-muted);
}

.run-stats {
  display: flex;
  gap: 0.5rem;
}

.stat-badge {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  font-size: 0.78rem;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  background: hsl(var(--muted-foreground) / 0.06);
  color: var(--af-muted);
}

/* Budget bar */
.budget-bar-container {
  margin-bottom: 1rem;
}

.budget-label {
  display: flex;
  justify-content: space-between;
  font-size: 0.78rem;
  color: var(--af-muted);
  margin-bottom: 0.3rem;
}

.budget-bar {
  height: 6px;
  background: hsl(var(--muted-foreground) / 0.08);
  border-radius: 3px;
  overflow: hidden;
}

.budget-fill {
  height: 100%;
  background: hsl(142 70% 45%);
  border-radius: 3px;
  transition: width 0.3s ease;
}

.budget-fill.warning { background: hsl(38 90% 50%); }
.budget-fill.danger { background: hsl(0 70% 50%); }

/* Pipeline flow */
.pipeline-flow {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  padding: 1rem;
  border: 1px solid var(--af-border);
  border-radius: 8px;
  overflow-x: auto;
  margin-bottom: 1rem;
}

.pipeline-step {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.2rem;
  padding: 0.5rem 0.6rem;
  border-radius: 8px;
  min-width: 72px;
  border: 1px solid transparent;
  transition: all 0.2s;
  position: relative;
  cursor: pointer;
}

.pipeline-step.completed {
  border-color: hsl(142 70% 45% / 0.25);
  background: hsl(142 70% 45% / 0.04);
}

.pipeline-step.running {
  border-color: hsl(var(--af-agents) / 0.4);
  background: hsl(var(--af-agents) / 0.08);
}

.pipeline-step.waiting_gate {
  border-color: hsl(38 90% 50% / 0.4);
  background: hsl(38 90% 50% / 0.08);
}

.pipeline-step.pending {
  opacity: 0.5;
}

.step-name { font-size: 0.73rem; font-weight: 500; color: var(--af-fg); }
.step-gate { font-size: 0.68rem; position: absolute; top: 2px; right: 2px; }
.step-pulse {
  position: absolute;
  top: 2px; left: 2px;
  width: 6px; height: 6px;
  border-radius: 50%;
  background: hsl(var(--af-agents));
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(1.3); }
  100% { opacity: 1; transform: scale(1); }
}

.step-connector {
  color: var(--af-border);
  display: flex;
  align-items: center;
}

/* Gate panel */
.gate-panel {
  padding: 0.75rem 1rem;
  border: 1px solid hsl(38 90% 50% / 0.3);
  border-radius: 8px;
  background: hsl(38 90% 50% / 0.04);
  margin-bottom: 1rem;
}

.gate-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.88rem;
  font-weight: 500;
  color: hsl(38 80% 35%);
  margin-bottom: 0.5rem;
}

.gate-actions {
  display: flex;
  gap: 0.4rem;
}

/* Session Log layout */
.session-log-body {
  display: flex;
  flex: 1;
  overflow: hidden;
  gap: 0.75rem;
}

.session-log-main {
  flex: 1;
  overflow-y: auto;
  min-width: 0;
}

/* Step timeline (right side) */
.step-timeline {
  width: 130px;
  flex-shrink: 0;
  border-left: 2px solid hsl(var(--muted-foreground) / 0.15);
  padding-left: 0.5rem;
  overflow-y: auto;
  padding-right: 0.25rem;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
}

.step-timeline-item {
  position: relative;
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  cursor: pointer;
  padding: 0.25rem 0;
  border-radius: 4px;
  transition: background 0.15s;
}

.step-timeline-item:hover {
  background: hsl(var(--muted-foreground) / 0.05);
}

.step-timeline-item.active .step-timeline-dot {
  background: hsl(var(--primary));
  box-shadow: 0 0 0 3px hsl(var(--primary) / 0.2);
}

.step-timeline-item.active .step-timeline-agent {
  color: hsl(var(--primary));
  font-weight: 600;
}

.step-timeline-dot {
  position: absolute;
  left: -7px;
  top: 0.55rem;
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: hsl(var(--muted-foreground) / 0.3);
  border: 2px solid var(--af-bg);
  flex-shrink: 0;
  transition: all 0.15s;
}

.step-timeline-content {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
  min-width: 0;
  padding-left: 0.15rem;
}

.step-timeline-agent {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.76rem;
  color: var(--af-fg);
  font-weight: 500;
  text-transform: capitalize;
  min-width: 0;
}

.step-timeline-agent span {
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.step-timeline-time {
  font-size: 0.68rem;
  color: var(--af-muted);
  font-family: monospace;
}

.step-timeline-retry {
  font-size: 0.62rem;
  font-weight: 600;
  color: hsl(38 80% 45%);
  background: hsl(38 80% 45% / 0.1);
  padding: 0 0.25rem;
  border-radius: 3px;
  margin-left: 0.15rem;
}

/* Expanded step card */
.expanded-step-card {
  position: absolute;
  top: calc(100% + 8px);
  left: 50%;
  transform: translateX(-50%);
  width: 200px;
  background: var(--af-card);
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.6rem 0.75rem;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08);
  z-index: 20;
}

.expanded-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-bottom: 0.4rem;
}

.expanded-name {
  flex: 1;
  font-size: 0.88rem;
  font-weight: 600;
  color: var(--af-fg);
}

.expanded-metrics {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  margin-bottom: 0.4rem;
}

.metric {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.83rem;
}

.metric-label { color: var(--af-muted); }
.metric-value { color: var(--af-fg); font-weight: 500; }

.expanded-actions {
  display: flex;
  justify-content: center;
}

.expanded-hint {
  font-size: 0.73rem;
  color: var(--af-muted);
}

/* Step retry badge */
.step-retry {
  position: absolute;
  bottom: 2px;
  right: 2px;
  font-size: 0.68rem;
  font-weight: 600;
  color: hsl(var(--af-error));
  background: hsl(var(--af-error) / 0.1);
  padding: 0 0.2rem;
  border-radius: 3px;
}

/* Session Log */
.session-log-panel {
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.75rem 1rem;
  margin-bottom: 1rem;
  display: flex;
  flex-direction: column;
  max-height: 570px;
}

.session-log-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  padding-right: 0.25rem;
}

.session-entry {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  padding: 0.4rem 0.5rem;
  border-radius: 6px;
  background: hsl(var(--muted-foreground) / 0.03);
}

.session-entry-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.78rem;
}

.session-profession {
  font-weight: 600;
  color: var(--af-fg);
  text-transform: capitalize;
}

.session-time {
  color: var(--af-muted);
  font-family: monospace;
  font-size: 0.72rem;
  margin-left: auto;
}

.session-text pre {
  margin: 0;
  padding: 0.3rem 0.4rem;
  background: hsl(var(--muted-foreground) / 0.04);
  border-radius: 4px;
  font-size: 0.82rem;
  line-height: 1.5;
  color: var(--af-fg);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 200px;
  overflow-y: auto;
}

.session-tool {
  border: 1px solid hsl(var(--primary) / 0.2);
  border-radius: 4px;
  background: hsl(var(--primary) / 0.04);
}

.session-tool.result {
  border-color: hsl(150 60% 45% / 0.3);
  background: hsl(150 60% 45% / 0.06);
}

.session-tool.unified {
  border-color: hsl(var(--primary) / 0.3);
  background: hsl(var(--primary) / 0.06);
}

.session-tool.unified .tool-body {
  display: flex;
  flex-direction: column;
  gap: 0.1rem;
}

.tool-badge {
  font-size: 0.65rem;
  font-weight: 500;
  padding: 0 0.3rem;
  border-radius: 3px;
  background: hsl(var(--primary) / 0.12);
  color: hsl(var(--primary));
  margin-left: auto;
}

.tool-badge.pending {
  background: hsl(38 80% 50% / 0.12);
  color: hsl(38 80% 35%);
}

.tool-header {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.3rem 0.5rem;
  font-size: 0.78rem;
  font-weight: 600;
  color: hsl(var(--primary));
}

.session-tool.result .tool-header {
  color: hsl(150 60% 35%);
}

.tool-details {
  padding: 0 0.5rem 0.4rem;
}

.tool-details summary {
  font-size: 0.72rem;
  color: var(--af-muted);
  cursor: pointer;
  user-select: none;
}

.tool-details pre {
  margin: 0.25rem 0 0;
  padding: 0.3rem 0.4rem;
  background: hsl(var(--muted-foreground) / 0.05);
  border-radius: 4px;
  font-size: 0.75rem;
  line-height: 1.4;
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.session-complete {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: hsl(150 60% 35%);
}

.session-error {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: hsl(0 72% 51%);
  background: hsl(0 72% 51% / 0.06);
  padding: 0.3rem 0.5rem;
  border-radius: 4px;
}

.session-warning {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: hsl(38 92% 50%);
  background: hsl(38 92% 50% / 0.08);
  padding: 0.3rem 0.5rem;
  border-radius: 4px;
}

.session-step {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: hsl(var(--af-agents));
  background: hsl(var(--af-agents) / 0.08);
  padding: 0.3rem 0.5rem;
  border-radius: 4px;
}

.session-step.completed {
  color: hsl(150 60% 35%);
  background: hsl(150 60% 35% / 0.08);
}

/* Cost Breakdown */
.cost-panel {
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.75rem 1rem;
  margin-bottom: 1rem;
}

.cost-chart {
  display: flex;
  align-items: center;
  gap: 1rem;
}

.pie-chart-container {
  flex-shrink: 0;
  width: 80px;
  height: 80px;
}

.pie-chart {
  width: 100%;
  height: 100%;
}

.pie-total {
  font-size: 5px;
  font-weight: 600;
  fill: var(--af-fg);
  font-family: 'JetBrains Mono', monospace;
}

.pie-slice-active {
  filter: drop-shadow(0 0 2px hsl(var(--primary) / 0.4));
}

.pie-legend {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  flex: 1;
  min-width: 0;
}

.pie-legend-item {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  font-size: 0.78rem;
  padding: 0.15rem 0.3rem;
  border-radius: 4px;
  transition: background 0.15s;
}

.pie-legend-item.active {
  background: hsl(var(--muted-foreground) / 0.06);
}

.pie-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.pie-prof {
  flex: 1;
  color: var(--af-fg);
  font-weight: 500;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.pie-val {
  color: var(--af-muted);
  font-family: 'JetBrains Mono', monospace;
  font-size: 0.72rem;
}

/* ─── Mobile Responsive ───────────────────────────────────────────────────── */

@media (max-width: 768px) {
  .agents-body {
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr auto;
    overflow-y: auto;
  }

  .runs-sidebar {
    max-height: 180px;
    border-bottom: 1px solid var(--af-border);
  }

  .pipeline-flow {
    padding: 0.5rem;
  }

  .expanded-step-card {
    position: static;
    transform: none;
    width: 100%;
    margin: 0.5rem 0;
  }
}

/* Modal */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}

.modal-content {
  background: var(--af-card);
  border: 1px solid var(--af-border);
  border-radius: 10px;
  padding: 1.25rem;
  width: 480px;
  max-width: 90vw;
  max-height: 80vh;
  overflow-y: auto;
}

.modal-content h3 {
  font-size: 0.98rem;
  font-weight: 600;
  margin-bottom: 1rem;
  color: var(--af-fg);
}

.form-group {
  margin-bottom: 0.75rem;
}

.form-group label {
  display: block;
  font-size: 0.83rem;
  font-weight: 500;
  color: var(--af-muted);
  margin-bottom: 0.3rem;
}

.form-group input, .form-group select, .form-group textarea {
  width: 100%;
  padding: 0.4rem 0.5rem;
  border: 1px solid var(--af-border);
  border-radius: 5px;
  background: var(--af-bg);
  color: var(--af-fg);
  font-size: 0.88rem;
  font-family: inherit;
}

.form-group textarea {
  resize: vertical;
}

.steps-builder {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.step-row {
  display: flex;
  gap: 0.4rem;
  align-items: center;
}

.step-input { flex: 1; }
.step-select { width: 100px; }

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1rem;
}
</style>
