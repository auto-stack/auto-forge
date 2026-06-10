import { computed, type Ref } from 'vue'

export interface ProfessionSegment {
  profession: string
  tokens: number
  color: string
}

export interface TooltipBarEntry {
  profession: string
  tokens: number
  percentage: number
  color: string
}

export const PROFESSION_PALETTE: Record<string, string> = {
  advisor:    '#6366f1',  // indigo
  architect:  '#8b5cf6',  // violet
  planner:    '#3b82f6',  // blue
  coder:      '#10b981',  // emerald
  tester:     '#f59e0b',  // amber
  reviewer:   '#ef4444',  // red
  documenter: '#06b6d4',  // cyan
  gofer:      '#64748b',  // slate
  _default:   '#94a3b8',  // gray
}

export function useProfessionSegments(professionTokens: Ref<Record<string, number>>) {
  const segments = computed<ProfessionSegment[]>(() =>
    Object.entries(professionTokens.value)
      .filter(([, tokens]) => tokens > 0)
      .map(([profession, tokens]) => ({
        profession,
        tokens,
        color: PROFESSION_PALETTE[profession] ?? PROFESSION_PALETTE._default,
      }))
      .sort((a, b) => b.tokens - a.tokens)
  )

  const totalUsed = computed(() =>
    segments.value.reduce((sum, s) => sum + s.tokens, 0)
  )

  const tooltipEntries = computed<TooltipBarEntry[]>(() => {
    const total = totalUsed.value || 1 // avoid division by zero
    return segments.value.map(s => ({
      profession: s.profession,
      tokens: s.tokens,
      percentage: Math.round((s.tokens / total) * 1000) / 10,
      color: s.color,
    }))
  })

  return { segments, totalUsed, tooltipEntries }
}
