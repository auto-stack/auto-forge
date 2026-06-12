import { describe, it, expect } from 'vitest'
import { ref } from 'vue'
import {
  useProfessionSegments,
  PROFESSION_PALETTE,
} from '../useProfessionSegments'

describe('useProfessionSegments', () => {
  it('TC-T23.1: filters > 0, sorts desc, assigns palette colors', () => {
    const professionTokens = ref({ architect: 5000, coder: 3000, tester: 1000 })
    const { segments } = useProfessionSegments(professionTokens)

    expect(segments.value).toHaveLength(3)
    expect(segments.value[0]).toEqual({
      profession: 'architect',
      tokens: 5000,
      color: '#8b5cf6',
    })
    expect(segments.value[1]).toEqual({
      profession: 'coder',
      tokens: 3000,
      color: '#10b981',
    })
    expect(segments.value[2]).toEqual({
      profession: 'tester',
      tokens: 1000,
      color: '#f59e0b',
    })
  })

  it('TC-T23.2: zero-token professions are filtered out', () => {
    const professionTokens = ref({ architect: 5000, coder: 0, tester: 1000 })
    const { segments } = useProfessionSegments(professionTokens)

    expect(segments.value).toHaveLength(2)
    expect(segments.value.find(s => s.profession === 'coder')).toBeUndefined()
  })

  it('TC-T23.3: empty professionTokens yields empty segments/total/tooltip', () => {
    const professionTokens = ref<Record<string, number>>({})
    const { segments, totalUsed, tooltipEntries } = useProfessionSegments(professionTokens)

    expect(segments.value).toEqual([])
    expect(totalUsed.value).toBe(0)
    expect(tooltipEntries.value).toEqual([])
  })

  it('TC-T23.4: unknown profession gets _default color', () => {
    const professionTokens = ref({ unknown_agent: 2000 })
    const { segments } = useProfessionSegments(professionTokens)

    expect(segments.value).toHaveLength(1)
    expect(segments.value[0].color).toBe('#94a3b8')
  })

  it('TC-T23.5: totalUsed sums all segment tokens', () => {
    const professionTokens = ref({ architect: 5000, coder: 3000, tester: 2000 })
    const { totalUsed } = useProfessionSegments(professionTokens)

    expect(totalUsed.value).toBe(10000)
  })

  it('TC-T23.6: tooltipEntries computes percentages to 1 decimal', () => {
    const professionTokens = ref({ architect: 7500, coder: 2500 })
    const { tooltipEntries } = useProfessionSegments(professionTokens)

    expect(tooltipEntries.value[0].percentage).toBe(75.0)
    expect(tooltipEntries.value[1].percentage).toBe(25.0)
  })

  it('TC-T23.7: reactivity — updating ref recomputes segments', () => {
    const professionTokens = ref<Record<string, number>>({ architect: 5000 })
    const { segments, totalUsed } = useProfessionSegments(professionTokens)

    expect(totalUsed.value).toBe(5000)
    professionTokens.value = { ...professionTokens.value, coder: 5000 }

    expect(totalUsed.value).toBe(10000)
    expect(segments.value).toHaveLength(2)
  })

  it('TC-T23.8: percentages sum to ~100% for multiple agents', () => {
    const professionTokens = ref({ a: 33, b: 33, c: 34 })
    const { tooltipEntries } = useProfessionSegments(professionTokens)

    const sum = tooltipEntries.value.reduce((s, e) => s + e.percentage, 0)
    expect(sum).toBeCloseTo(100, 0)
  })

  it('TC-T23.9: single profession at 100%', () => {
    const professionTokens = ref({ coder: 9999 })
    const { tooltipEntries } = useProfessionSegments(professionTokens)

    expect(tooltipEntries.value).toHaveLength(1)
    expect(tooltipEntries.value[0].percentage).toBe(100.0)
  })

  it('TC-T23.10: very small profession still appears in segments', () => {
    const professionTokens = ref({ architect: 99900, gofer: 100 })
    const { segments, tooltipEntries } = useProfessionSegments(professionTokens)

    expect(segments.value).toHaveLength(2)
    expect(tooltipEntries.value[1].percentage).toBe(0.1) // 100/100000 * 100 = 0.1
  })

  it('TC-T23.11: all 8 known professions get their palette color', () => {
    const known = [
      'advisor', 'architect', 'planner', 'coder',
      'tester', 'reviewer', 'documenter', 'gofer',
    ]
    const tokens = Object.fromEntries(known.map((k, i) => [k, (i + 1) * 100]))
    const professionTokens = ref(tokens)
    const { segments } = useProfessionSegments(professionTokens)

    const palette: Record<string, string> = {
      advisor: '#6366f1',
      architect: '#8b5cf6',
      planner: '#3b82f6',
      coder: '#10b981',
      tester: '#f59e0b',
      reviewer: '#ef4444',
      documenter: '#06b6d4',
      gofer: '#64748b',
    }
    for (const seg of segments.value) {
      expect(seg.color).toBe(palette[seg.profession])
    }
  })
})
