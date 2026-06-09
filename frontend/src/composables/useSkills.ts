import { ref, computed } from 'vue'
import { authFetch } from './useAuth'

const API_BASE = '/api/forge/config/skills'

export interface SkillDto {
  id: string
  name: string
  description: string
  granted_tools: string[]
  prompt_fragment: string
  extra_turns: number
  extra_token_budget: number
}

// Singleton state
const _skills = ref<SkillDto[]>([])
const _loading = ref(false)
const _error = ref<string | null>(null)

export function useSkills() {
  const skills = computed(() => _skills.value)
  const loading = computed(() => _loading.value)
  const error = computed(() => _error.value)

  async function loadSkills() {
    _loading.value = true
    _error.value = null
    try {
      const resp = await authFetch(API_BASE)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      _skills.value = await resp.json()
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
    } finally {
      _loading.value = false
    }
  }

  async function createSkill(skill: SkillDto): Promise<boolean> {
    _error.value = null
    try {
      const resp = await authFetch(API_BASE, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(skill),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const created = await resp.json()
      _skills.value.push(created)
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function updateSkill(id: string, skill: SkillDto): Promise<boolean> {
    _error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(id)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(skill),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const updated = await resp.json()
      const idx = _skills.value.findIndex(s => s.id === id)
      if (idx >= 0) _skills.value[idx] = updated
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function deleteSkill(id: string): Promise<boolean> {
    _error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(id)}`, { method: 'DELETE' })
      if (resp.status !== 204) throw new Error(`HTTP ${resp.status}`)
      _skills.value = _skills.value.filter(s => s.id !== id)
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function resetDefaults(): Promise<boolean> {
    _error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/reset-defaults`, { method: 'POST' })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      _skills.value = await resp.json()
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  return {
    skills,
    loading,
    error,
    loadSkills,
    createSkill,
    updateSkill,
    deleteSkill,
    resetDefaults,
  }
}
