<template>
  <div
    class="segmented-budget-bar"
    :class="budgetClass"
    tabindex="0"
    role="progressbar"
    :aria-valuenow="totalUsed"
    :aria-valuemax="totalBudget"
    :aria-label="`Token budget: ${formatCompact(totalUsed)} of ${formatCompact(totalBudget)} used`"
    @mouseenter="onMouseEnter"
    @mouseleave="onMouseLeave"
    @focusin="onMouseEnter"
    @focusout="onMouseLeave"
  >
    <div class="budget-label-row">
      <span>Budget</span>
      <span>{{ formatCompact(totalUsed) }} / {{ formatCompact(totalBudget) }}</span>
    </div>

    <div v-if="segments.length === 0" class="empty-placeholder">
      No token data yet
    </div>
    <div
      v-else
      ref="containerRef"
      class="segments-container"
      @mousemove="onContainerMouseMove"
    >
      <div
        v-for="seg in segments"
        :key="seg.profession"
        class="segment"
        :style="segmentStyle(seg)"
      />
    </div>

    <!-- Hover tooltip — positioned below, aligned to the hovered segment -->
    <Transition name="tooltip-fade">
      <div
        v-if="showTooltip && tooltipEntries.length > 0"
        class="breakdown-tooltip"
        :style="tooltipPositionStyle"
        role="tooltip"
        @mouseenter="onTooltipEnter"
        @mouseleave="onMouseLeave"
      >
        <div class="tooltip-title">Token Breakdown</div>
        <div class="tooltip-divider" />

        <div
          v-for="entry in tooltipEntries"
          :key="entry.profession"
          class="tooltip-row"
          :class="{ highlight: entry.profession === (props.segments[hoveredIdx]?.profession) }"
        >
          <span class="color-dot" :style="{ background: entry.color }" />
          <span class="profession-name">{{ entry.profession }}</span>
          <div class="mini-bar-track">
            <div
              class="mini-bar-fill"
              :style="{ width: entry.percentage + '%', background: entry.color }"
            />
          </div>
          <span class="percentage-label">{{ entry.percentage }}%</span>
        </div>

        <div class="tooltip-divider" />
        <div class="tooltip-total">
          {{ formatCompact(totalUsed) }} / {{ formatCompact(totalBudget) }}
        </div>
      </div>
    </Transition>
  </div>
</template>

<script setup lang="ts">
import { computed, ref } from 'vue'
import type { ProfessionSegment, TooltipBarEntry } from '@/composables/useProfessionSegments'

const props = withDefaults(defineProps<{
  segments: ProfessionSegment[]
  totalBudget: number
  totalUsed: number
  tooltipEntries: TooltipBarEntry[]
  warnThreshold?: number
  dangerThreshold?: number
}>(), {
  warnThreshold: 0.7,
  dangerThreshold: 0.9,
})

// Budget warning class
const budgetClass = computed(() => {
  if (!props.totalBudget) return ''
  const ratio = props.totalUsed / props.totalBudget
  if (ratio >= (props.dangerThreshold ?? 0.9)) return 'budget-danger'
  if (ratio >= (props.warnThreshold ?? 0.7)) return 'budget-warn'
  return ''
})

// Segment style: proportional to totalBudget (not totalUsed)
function segmentStyle(seg: ProfessionSegment) {
  return {
    width: props.totalBudget > 0
      ? `${(seg.tokens / props.totalBudget) * 100}%`
      : '0%',
    backgroundColor: seg.color,
    minWidth: seg.tokens > 0 ? '2px' : '0',
  }
}

// Tooltip visibility with delays + segment-aware positioning
const showTooltip = ref(false)
const containerRef = ref<HTMLElement | null>(null)
const hoveredIdx = ref(-1)
let showTimer: ReturnType<typeof setTimeout> | null = null
let hideTimer: ReturnType<typeof setTimeout> | null = null

function cancelTimers() {
  if (showTimer) { clearTimeout(showTimer); showTimer = null }
  if (hideTimer) { clearTimeout(hideTimer); hideTimer = null }
}

function resolveHoveredSegment(clientX: number) {
  if (!containerRef.value || props.segments.length === 0) {
    hoveredIdx.value = -1
    return
  }
  const rect = containerRef.value.getBoundingClientRect()
  const ratio = Math.max(0, Math.min(1, (clientX - rect.left) / rect.width))

  let cumulative = 0
  for (let i = 0; i < props.segments.length; i++) {
    const segWidth = props.totalBudget > 0 ? props.segments[i].tokens / props.totalBudget : 0
    if (ratio >= cumulative && ratio < cumulative + segWidth) {
      hoveredIdx.value = i
      return
    }
    cumulative += segWidth
  }
  hoveredIdx.value = -1
}

function onMouseEnter(e?: MouseEvent | FocusEvent) {
  if (e && 'clientX' in e) resolveHoveredSegment(e.clientX)
  cancelTimers()
  showTimer = setTimeout(() => {
    showTooltip.value = true
  }, 80)
}

function onContainerMouseMove(e: MouseEvent) {
  resolveHoveredSegment(e.clientX)
}

function onMouseLeave() {
  cancelTimers()
  hideTimer = setTimeout(() => {
    showTooltip.value = false
    hoveredIdx.value = -1
  }, 150)
}

function onTooltipEnter() {
  // Cancel hide when mouse moves into tooltip itself
  cancelTimers()
}

const tooltipPositionStyle = computed(() => {
  return { left: '50%', transform: 'translateX(-50%)' }
})

function formatCompact(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return `${n}`
}
</script>

<style scoped>
.segmented-budget-bar {
  position: relative;
  margin-bottom: 1rem;
  outline: none;
}

.segmented-budget-bar:focus-visible {
  outline: 2px solid hsl(var(--primary) / 0.5);
  outline-offset: 2px;
}

.budget-label-row {
  display: flex;
  justify-content: space-between;
  font-size: 0.78rem;
  color: var(--af-muted);
  margin-bottom: 0.3rem;
}

.segments-container {
  display: flex;
  height: 12px;
  border-radius: 6px;
  background: hsl(var(--muted-foreground) / 0.08);
  overflow: hidden;
  transition: box-shadow 0.5s ease-in-out;
}

.segment {
  height: 100%;
  transition: width 0.3s ease-out;
}

.segment:first-child {
  border-radius: 6px 0 0 6px;
}

.segment:last-child {
  border-radius: 0 6px 6px 0;
}

.segment:only-child {
  border-radius: 6px;
}

/* Budget warning borders */
.budget-warn .segments-container {
  box-shadow: 0 0 0 2px rgba(245, 158, 11, 0.3);
}

.budget-danger .segments-container {
  box-shadow: 0 0 0 2px rgba(239, 68, 68, 0.4);
}

.empty-placeholder {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 12px;
  background: hsl(var(--muted-foreground) / 0.08);
  border-radius: 6px;
  color: var(--af-muted);
  font-size: 0.78rem;
}

/* Tooltip — positioned below the bar to avoid clipping by panel top edge */
.breakdown-tooltip {
  position: absolute;
  top: calc(100% + 8px);
  left: 50%;
  transform: translateX(-50%);
  z-index: 50;
  min-width: 420px;
  max-width: 540px;
  background: var(--af-bg);
  border: 1px solid var(--af-border);
  border-radius: 12px;
  padding: 1.125rem;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.15);
  pointer-events: auto;
}

.tooltip-title {
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--af-fg);
  margin-bottom: 0.75rem;
}

.tooltip-divider {
  height: 1px;
  background: var(--af-border);
  margin: 0.6rem 0;
}

.tooltip-row {
  display: grid;
  grid-template-columns: 15px 120px 1fr 72px;
  align-items: center;
  gap: 0.6rem;
  padding: 0.3rem 0.45rem;
  border-radius: 6px;
  transition: background 100ms ease;
}

.tooltip-row.highlight {
  background: hsl(var(--primary) / 0.08);
  font-weight: 600;
}

.color-dot {
  width: 12px;
  height: 12px;
  border-radius: 50%;
}

.profession-name {
  font-size: 0.75rem;
  color: var(--af-fg);
  font-weight: 500;
  text-transform: capitalize;
}

.mini-bar-track {
  height: 9px;
  background: hsl(var(--muted-foreground) / 0.08);
  border-radius: 4.5px;
  overflow: hidden;
}

.mini-bar-fill {
  height: 100%;
  border-radius: 4.5px;
  transition: width 0.2s ease;
}

.percentage-label {
  font-size: 0.72rem;
  color: var(--af-muted);
  text-align: right;
  font-family: 'JetBrains Mono', monospace;
}

.tooltip-total {
  font-size: 0.75rem;
  color: var(--af-muted);
  text-align: center;
  font-family: 'JetBrains Mono', monospace;
}

/* Transitions */
.tooltip-fade-enter-active { transition: opacity 150ms ease; }
.tooltip-fade-leave-active { transition: opacity 100ms ease; }
.tooltip-fade-enter-from,
.tooltip-fade-leave-to { opacity: 0; }
</style>
