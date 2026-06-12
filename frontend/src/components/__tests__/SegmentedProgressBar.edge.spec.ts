import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import SegmentedProgressBar from '../SegmentedProgressBar.vue'
import type { ProfessionSegment, TooltipBarEntry } from '@/composables/useProfessionSegments'

const palette: Record<string, string> = {
  advisor: '#6366f1', architect: '#8b5cf6', planner: '#3b82f6',
  coder: '#10b981', tester: '#f59e0b', reviewer: '#ef4444',
  documenter: '#06b6d4', gofer: '#64748b', _default: '#94a3b8',
}

function makeSegments(map: Record<string, number>): ProfessionSegment[] {
  return Object.entries(map)
    .filter(([, t]) => t > 0)
    .map(([p, t]) => ({ profession: p, tokens: t, color: palette[p] ?? palette._default }))
    .sort((a, b) => b.tokens - a.tokens)
}

function makeTooltipEntries(segments: ProfessionSegment[]): TooltipBarEntry[] {
  const total = segments.reduce((s, seg) => s + seg.tokens, 0) || 1
  return segments.map(s => ({
    ...s,
    percentage: Math.round((s.tokens / total) * 1000) / 10,
  }))
}

function mountBar(overrides: {
  segments?: ProfessionSegment[]
  totalBudget?: number
  totalUsed?: number
  tooltipEntries?: TooltipBarEntry[]
} = {}) {
  const segments = overrides.segments ?? makeSegments({ coder: 1000 })
  const totalUsed = overrides.totalUsed ?? segments.reduce((s, seg) => s + seg.tokens, 0)
  return mount(SegmentedProgressBar, {
    props: {
      segments,
      totalBudget: overrides.totalBudget ?? 10000,
      totalUsed,
      tooltipEntries: overrides.tooltipEntries ?? makeTooltipEntries(segments),
    },
    attachTo: document.body,
  })
}

describe('SegmentedProgressBar — edge cases', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  // TC-26.1
  it('single profession fills full bar', () => {
    const segments = makeSegments({ coder: 10000 })
    const wrapper = mountBar({ segments, totalBudget: 10000, totalUsed: 10000 })
    const segmentDivs = wrapper.findAll('.segment')
    expect(segmentDivs).toHaveLength(1)
    // 10000 / 10000 = 100%
    expect(segmentDivs[0].attributes('style')).toContain('width: 100%')
  })

  // TC-26.2
  it('tiny profession gets 2px min-width', () => {
    const segments = makeSegments({ architect: 99990, gofer: 10 })
    const wrapper = mountBar({ segments, totalBudget: 100000, totalUsed: 100000 })
    const segmentDivs = wrapper.findAll('.segment')
    expect(segmentDivs).toHaveLength(2)
    // gofer is the last segment (lowest tokens) — verify it has min-width: 2px
    const lastSeg = segmentDivs[segmentDivs.length - 1]
    expect(lastSeg.attributes('style')).toContain('min-width: 2px')
  })

  // TC-26.3
  it('budget=0 — no warning classes', () => {
    const wrapper = mountBar({ totalBudget: 0, totalUsed: 0, segments: [], tooltipEntries: [] })
    const bar = wrapper.find('.segmented-budget-bar')
    expect(bar.classes()).not.toContain('budget-warn')
    expect(bar.classes()).not.toContain('budget-danger')
  })

  // TC-26.4
  it('budget=0 with tokens — segments still render', () => {
    const segments = makeSegments({ coder: 5000 })
    const wrapper = mountBar({ segments, totalBudget: 0, totalUsed: 5000 })
    // Should still render segments (width will be 0% but min-width keeps them visible)
    expect(wrapper.findAll('.segment')).toHaveLength(1)
  })

  // TC-26.5
  it('zero tokens all professions — empty state', () => {
    const wrapper = mountBar({
      segments: [],
      totalBudget: 10000,
      totalUsed: 0,
      tooltipEntries: [],
    })
    expect(wrapper.find('.empty-placeholder').exists()).toBe(true)
    expect(wrapper.findAll('.segment')).toHaveLength(0)
  })

  // TC-26.6
  it('percentage rounding for 33.3/33.3/33.4 split', () => {
    const segments = makeSegments({ a: 333, b: 333, c: 334 })
    const entries = makeTooltipEntries(segments)
    // Verify the percentages round correctly
    expect(entries.find(e => e.profession === 'a')?.percentage).toBe(33.3)
    expect(entries.find(e => e.profession === 'b')?.percentage).toBe(33.3)
    expect(entries.find(e => e.profession === 'c')?.percentage).toBe(33.4)

    // Verify they sum close to 100
    const sum = entries.reduce((s, e) => s + e.percentage, 0)
    expect(sum).toBeCloseTo(100, 0)
  })

  // TC-26.7
  it('tooltip entry rows render with color dots and mini-bars', async () => {
    const segments = makeSegments({ coder: 7000, tester: 3000 })
    const wrapper = mountBar({ segments, totalBudget: 10000, totalUsed: 10000 })

    // Show tooltip
    await wrapper.find('.segmented-budget-bar').trigger('mouseenter')
    vi.advanceTimersByTime(80)
    await wrapper.vm.$nextTick()

    const tooltip = wrapper.find('.breakdown-tooltip')
    expect(tooltip.exists()).toBe(true)

    const rows = tooltip.findAll('.tooltip-row')
    expect(rows).toHaveLength(2)

    // Each row should have a color dot and mini-bar
    for (const row of rows) {
      expect(row.find('.color-dot').exists()).toBe(true)
      expect(row.find('.mini-bar-track').exists()).toBe(true)
      expect(row.find('.mini-bar-fill').exists()).toBe(true)
      expect(row.find('.percentage-label').exists()).toBe(true)
    }
  })

  // TC-26.8
  it('tooltip total row shows formatted used/budget', async () => {
    const segments = makeSegments({ coder: 15000 })
    const wrapper = mountBar({ segments, totalBudget: 50000, totalUsed: 15000 })

    // Show tooltip
    await wrapper.find('.segmented-budget-bar').trigger('mouseenter')
    vi.advanceTimersByTime(80)
    await wrapper.vm.$nextTick()

    const total = wrapper.find('.tooltip-total')
    expect(total.exists()).toBe(true)
    // 15000 → "15.0k", 50000 → "50.0k"
    expect(total.text()).toContain('15.0k')
    expect(total.text()).toContain('50.0k')
  })
})
