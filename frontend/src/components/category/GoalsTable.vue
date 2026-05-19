<template>
  <div class="goals-tree-wrapper">
    <div v-if="items.length === 0" class="empty-state">
      <Inbox :size="28" />
      <span>No goals yet</span>
      <span class="empty-hint">Click "Add" above to create one</span>
    </div>
    <div v-else class="goals-tree">
      <div
        v-for="row in treeRows"
        :key="row.item.id"
        class="goal-node"
        :class="{
          'is-root': row.level === 0,
          'is-sub': row.level > 0,
          'has-children': row.hasChildren,
        }"
        @click="onRowClick(row.item)"
      >
        <!-- Indent spacer -->
        <div class="node-indent" :style="{ width: row.level * 1.25 + 'rem' }"></div>

        <!-- Toggle area: children count + chevron, fixed width -->
        <div
          v-if="row.hasChildren"
          class="node-toggle-area"
          :class="{ expanded: treeExpanded.has(row.item.id) }"
          :title="row.childCount + ' sub-goals'"
          @click="onToggleClick($event, row)"
        >
          <span
            v-if="row.level === 0 && row.childCount > 0"
            class="node-children-count"
          >
            {{ row.childCount }}
          </span>
          <span class="node-chevron">
            <ChevronRight :size="14" />
          </span>
        </div>

        <!-- Left: indent + toggle + ID -->
        <div class="node-left">
          <span class="node-id">{{ row.item.id }}</span>
        </div>

        <!-- Right: title + meta -->
        <div class="node-right">
          <span class="node-title">{{ row.item.title }}</span>

          <template v-if="row.level === 0">
            <span v-if="row.item.priority" class="node-priority" :class="row.item.priority">
              {{ row.item.priority }}
            </span>
            <StatusBadge :status="row.item.status" size="sm" />
            <span v-if="row.item.tags?.length" class="node-tags">
              <span
                v-for="tag in row.item.tags.slice(0, 2)"
                :key="tag"
                class="tag-chip"
                :class="parseTag(tag).type"
                :title="parseTag(tag).full"
              >{{ parseTag(tag).value }}</span>
              <span v-if="row.item.tags.length > 2" class="tag-more">+{{ row.item.tags.length - 2 }}</span>
            </span>
          </template>

          <template v-else>
            <StatusBadge :status="row.item.status" size="sm" />
            <span v-if="row.item.tags?.length" class="node-tags">
              <span
                v-for="tag in row.item.tags.slice(0, 2)"
                :key="tag"
                class="tag-chip"
                :class="parseTag(tag).type"
                :title="parseTag(tag).full"
              >{{ parseTag(tag).value }}</span>
            </span>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from 'vue'
import type { SpecItem } from '@/types/specs'
import StatusBadge from '@/components/StatusBadge.vue'
import { Inbox, ChevronRight } from 'lucide-vue-next'

const props = defineProps<{
  items: SpecItem[]
  project: string
}>()

const emit = defineEmits<{
  'open-detail': [item: SpecItem]
}>()

function parseTag(tag: string): { type: string; value: string; full: string } {
  const idx = tag.indexOf(':')
  if (idx > 0) {
    return {
      type: tag.slice(0, idx),
      value: tag.slice(idx + 1),
      full: tag,
    }
  }
  return { type: 'other', value: tag, full: tag }
}

// ─── Tree state ──────────────────────────────────────────────

const treeExpanded = ref<Set<string>>(new Set())

function getParentId(id: string): string | null {
  const m = id.match(/^(G\d+)\./)
  return m ? m[1] : null
}

function buildChildMap(items: SpecItem[]): Map<string, SpecItem[]> {
  const itemMap = new Map(items.map(i => [i.id, i]))
  const cmap = new Map<string, SpecItem[]>()

  for (const item of items) {
    const parentId = getParentId(item.id)
    if (parentId && itemMap.has(parentId)) {
      if (!cmap.has(parentId)) cmap.set(parentId, [])
      cmap.get(parentId)!.push(item)
    }
  }

  for (const [, children] of cmap) {
    children.sort((a, b) => a.id.localeCompare(b.id))
  }

  return cmap
}

const childrenMap = computed(() => buildChildMap(props.items))

interface TreeRow {
  item: SpecItem
  level: number
  hasChildren: boolean
  childCount: number
}

const treeRows = computed(() => {
  const result: TreeRow[] = []
  const expanded = treeExpanded.value
  const cmap = childrenMap.value

  const roots = props.items
    .filter(i => !getParentId(i.id))
    .sort((a, b) => a.id.localeCompare(b.id))

  function addNode(item: SpecItem, level: number) {
    const children = cmap.get(item.id) || []
    result.push({
      item,
      level,
      hasChildren: children.length > 0,
      childCount: children.length,
    })
    if (expanded.has(item.id)) {
      for (const child of children) {
        addNode(child, level + 1)
      }
    }
  }

  for (const root of roots) {
    addNode(root, 0)
  }

  return result
})

function toggleTree(id: string) {
  const next = new Set(treeExpanded.value)
  if (next.has(id)) {
    next.delete(id)
  } else {
    next.add(id)
  }
  treeExpanded.value = next
}

function onToggleClick(e: MouseEvent, row: TreeRow) {
  if (row.hasChildren) {
    e.stopPropagation()
    toggleTree(row.item.id)
  }
  // else: let click bubble through to open detail
}

function onRowClick(item: SpecItem) {
  emit('open-detail', item)
}
</script>

<style scoped>
.goals-tree-wrapper {
  overflow-x: auto;
}

.goals-tree {
  display: flex;
  flex-direction: column;
  font-size: 0.93rem;
}

/* ── Goal Node ─────────────────────────────────────────────── */

.goal-node {
  display: flex;
  align-items: stretch;
  cursor: pointer;
  border-bottom: 1px solid var(--af-border);
  transition: background 0.12s;
  min-height: 44px;
}

.goal-node:hover {
  background: hsl(var(--muted-foreground) / 0.03);
}

/* Root node */
.goal-node.is-root {
  font-weight: 500;
  border-left: 3px solid transparent;
}
.goal-node.is-root:hover {
  border-left-color: hsl(var(--primary));
}

/* SubGoal node */
.goal-node.is-sub {
  font-weight: 400;
  font-size: 0.88rem;
  background: hsl(var(--muted-foreground) / 0.02);
  border-bottom-color: hsl(var(--muted-foreground) / 0.06);
}

/* Left column: indent + toggle + ID */
.node-left {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex-shrink: 0;
  padding-left: 1rem;
  padding-right: 0.5rem;
}

/* Right column: title + meta */
.node-right {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  flex: 1;
  min-width: 0;
  padding-right: 1rem;
}

/* Indent spacer */
.node-indent {
  flex-shrink: 0;
}

/* Chevron toggle */
.node-toggle-area {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 0.15rem;
  width: 2.5rem;
  align-self: stretch;
  flex-shrink: 0;
  cursor: pointer;
  border-radius: 4px;
  transition: background 0.12s;
}
.node-toggle-area:hover {
  background: hsl(var(--muted-foreground) / 0.08);
}
.node-toggle-area.expanded .node-chevron {
  transform: rotate(90deg);
}
.node-chevron {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  color: var(--af-muted);
  transition: transform 0.15s;
}

/* ID */
.node-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-weight: 600;
  color: var(--af-muted);
  font-size: 0.8rem;
  white-space: nowrap;
  flex-shrink: 0;
}

/* Title */
.node-title {
  color: var(--af-fg);
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  padding-right: 0.5rem;
}

/* Priority pill */
.node-priority {
  display: inline-flex;
  padding: 0.1rem 0.35rem;
  font-size: 0.7rem;
  font-weight: 700;
  border-radius: 4px;
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-muted);
  flex-shrink: 0;
}
.node-priority.P0 {
  background: hsl(0 72% 51% / 0.12);
  color: #ef4444;
}
.node-priority.P1 {
  background: hsl(38 92% 50% / 0.12);
  color: #f59e0b;
}
.node-priority.P2 {
  background: hsl(217 91% 60% / 0.12);
  color: #3b82f6;
}

/* Tags */
.node-tags {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  flex-shrink: 0;
}
.tag-chip {
  display: inline-flex;
  align-items: center;
  padding: 0.1rem 0.3rem;
  font-size: 0.68rem;
  border-radius: 4px;
  border: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.06);
  color: var(--af-muted);
  white-space: nowrap;
  cursor: help;
}
.tag-chip.stack {
  background: hsl(200 80% 50% / 0.1);
  color: hsl(200 70% 45%);
  border-color: hsl(200 70% 50% / 0.25);
}
.tag-chip.module {
  background: hsl(280 60% 55% / 0.1);
  color: hsl(280 50% 50%);
  border-color: hsl(280 50% 50% / 0.25);
}
.tag-more {
  font-size: 0.68rem;
  color: var(--af-muted);
}

/* Children count */
.node-children-count {
  display: inline-flex;
  padding: 0.1rem 0.4rem;
  font-size: 0.7rem;
  font-weight: 600;
  border-radius: 999px;
  background: hsl(var(--primary) / 0.08);
  color: hsl(var(--primary));
  flex-shrink: 0;
}

/* Empty state */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.4rem;
  padding: 2.5rem 1rem;
  color: var(--af-muted);
  font-size: 0.93rem;
}
.empty-state svg {
  color: hsl(var(--muted-foreground) / 0.3);
  margin-bottom: 0.3rem;
}
.empty-hint {
  font-size: 0.83rem;
  color: hsl(var(--muted-foreground) / 0.6);
}
</style>
