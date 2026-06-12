import { describe, it, expect, vi, beforeEach } from 'vitest'
import { mount } from '@vue/test-utils'
import SegmentedProgressBar from '../SegmentedProgressBar.vue'
import type { ProfessionSegment, TooltipBarEntry } from '@/composables/useProfessionSegments'

function mountBar(overrides: Record<string, unknown> = {}) {
  const defaultSegments: ProfessionSegment[] = [
    { profession: 'architect', tokens: 5000, color: '#8b5cf6' },
    { profession: 'coder', tokens: 3000, color: '#10b981' },
    { profession: 'tester', tokens: 2000, color: '#f59e0b' },
  ]
  const defaultTooltipEntries: TooltipBarEntry[] = [
    { profession: 'architect', tokens: 5000, color: '#8b5cf6', percentage: 50.0 },
    { profession: 'coder', tokens: 3000, color: '#10b981', percentage: 30.0 },
    { profession: 'tester', tokens: 2000, color: '#f59e0b', percentage: 20.0 },
  ]

  return mount(SegmentedProgressBar, {
    props: {
      segments: defaultSegments,
      totalBudget: 100000,
      totalUsed: 10000,
      tooltipEntries: defaultTooltipEntries,
      ...overrides,
    },
    attachTo: document.body,
  })
}

describe('SegmentedProgressBar', () => {
  beforeEach(() => {
    vi.useFakeTimers()
  })

  it('TC-24.1: renders correct number of segment divs', () => {
    const wrapper = mountBar()
    const segments = wrapper.findAll('.segment')
    expect(segments).toHaveLength(3)
  })

  it('TC-24.2: segment widths proportional to totalBudget', () => {
    const wrapper = mountBar()
    const segments = wrapper.findAll('.segment')
    expect(segments[0].attributes('style')).toContain('width: 5%')
    expect(segments[1].attributes('style')).toContain('width: 3%')
    expect(segments[2].attributes('style')).toContain('width: 2%')
  })

  it('TC-24.3: empty state placeholder when no segments', () => {
    const wrapper = mountBar({
      segments: [],
      tooltipEntries: [],
      totalUsed: 0,
    })
    expect(wrapper.find('.empty-placeholder').exists()).toBe(true)
    expect(wrapper.find('.segments-container').exists()).toBe(false)
    expect(wrapper.text()).toContain('No token data yet')
  })

  it('TC-24.4: budget warn class at 70% threshold', () => {
    const wrapper = mountBar({ totalUsed: 70000, totalBudget: 100000 })
    const bar = wrapper.find('.segmented-budget-bar')
    expect(bar.classes()).toContain('budget-warn')
  })

  it('TC-24.5: budget danger class at 90% threshold', () => {
    const wrapper = mountBar({ totalUsed: 92000, totalBudget: 100000 })
    const bar = wrapper.find('.segmented-budget-bar')
    expect(bar.classes()).toContain('budget-danger')
  })

  it('TC-24.6: no budget class when under 70%', () => {
    const wrapper = mountBar({ totalUsed: 50000, totalBudget: 100000 })
    const bar = wrapper.find('.segmented-budget-bar')
    expect(bar.classes()).not.toContain('budget-warn')
    expect(bar.classes()).not.toContain('budget-danger')
  })

  it('TC-24.7: no budget class when totalBudget is 0', () => {
    const wrapper = mountBar({
      totalBudget: 0,
      totalUsed: 0,
      segments: [],
      tooltipEntries: [],
    })
    const bar = wrapper.find('.segmented-budget-bar')
    expect(bar.classes()).not.toContain('budget-warn')
    expect(bar.classes()).not.toContain('budget-danger')
  })

  it('TC-24.8: tooltip shows on mouseenter, hides on mouseleave', async () => {
    const wrapper = mountBar()
    const bar = wrapper.find('.segmented-budget-bar')

    expect(wrapper.find('.breakdown-tooltip').exists()).toBe(false)

    await bar.trigger('mouseenter')
    vi.advanceTimersByTime(80)
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.breakdown-tooltip').exists()).toBe(true)

    await bar.trigger('mouseleave')
    vi.advanceTimersByTime(150)
    await wrapper.vm.$nextTick()
    expect(wrapper.find('.breakdown-tooltip').exists()).toBe(false)
  })

  it('TC-24.9: min-width on tiny segments', () => {
    const wrapper = mountBar({
      segments: [
        { profession: 'architect', tokens: 99900, color: '#8b5cf6' },
        { profession: 'gofer', tokens: 10, color: '#64748b' },
      ],
      tooltipEntries: [
        { profession: 'architect', tokens: 99900, color: '#8b5cf6', percentage: 99.99 },
        { profession: 'gofer', tokens: 10, color: '#64748b', percentage: 0.01 },
      ],
      totalUsed: 99910,
      totalBudget: 100000,
    })
    const segments = wrapper.findAll('.segment')
    expect(segments[0].attributes('style')).toContain('min-width: 2px')
    expect(segments[1].attributes('style')).toContain('min-width: 2px')
  })

  it('TC-24.10: accessibility — role=progressbar with aria attributes', () => {
    const wrapper = mountBar({ totalUsed: 10000, totalBudget: 100000 })
    const bar = wrapper.find('.segmented-budget-bar')
    expect(bar.attributes('role')).toBe('progressbar')
    expect(bar.attributes('aria-valuenow')).toBe('10000')
    expect(bar.attributes('aria-valuemax')).toBe('100000')
    expect(bar.attributes('aria-label')).toContain('10.0k')
    expect(bar.attributes('aria-label')).toContain('100.0k')
  })

  it('TC-24.11: custom warn/danger thresholds override defaults', () => {
    const wrapper = mountBar({
      totalUsed: 60000,
      totalBudget: 100000,
      warnThreshold: 0.5,
      dangerThreshold: 0.8,
    })
    const bar = wrapper.find('.segmented-budget-bar')
    expect(bar.classes()).toContain('budget-warn')
    expect(bar.classes()).not.toContain('budget-danger')
  })
})
