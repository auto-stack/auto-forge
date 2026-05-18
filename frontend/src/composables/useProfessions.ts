import { ref, computed } from 'vue'

const API_BASE = '/api/forge/config/professions'

export interface ProfessionDto {
  id: string
  name: string
  phase: string
  owned_sections: string[]
  readable_sections: string[]
  allowed_tools: string[]
  handoff_to: string[]
  dispatchable_to: string[]
  approval_gates: string[]
  max_turns: number
  token_budget: number
  base_skills: string[]
}

// Singleton state
const _professions = ref<ProfessionDto[]>([])
const _loading = ref(false)
const _error = ref<string | null>(null)

export function useProfessions() {
  const professions = computed(() => _professions.value)
  const loading = computed(() => _loading.value)
  const error = computed(() => _error.value)

  async function loadProfessions() {
    _loading.value = true
    _error.value = null
    try {
      const resp = await fetch(API_BASE)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      _professions.value = await resp.json()
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
    } finally {
      _loading.value = false
    }
  }

  async function createProfession(prof: ProfessionDto): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(API_BASE, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(prof),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const created = await resp.json()
      _professions.value.push(created)
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function updateProfession(id: string, prof: ProfessionDto): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/${encodeURIComponent(id)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(prof),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const updated = await resp.json()
      const idx = _professions.value.findIndex(p => p.id === id)
      if (idx >= 0) _professions.value[idx] = updated
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function deleteProfession(id: string): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/${encodeURIComponent(id)}`, { method: 'DELETE' })
      if (resp.status !== 204) throw new Error(`HTTP ${resp.status}`)
      _professions.value = _professions.value.filter(p => p.id !== id)
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function resetDefaults(): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/reset-defaults`, { method: 'POST' })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      _professions.value = await resp.json()
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  return {
    professions,
    loading,
    error,
    loadProfessions,
    createProfession,
    updateProfession,
    deleteProfession,
    resetDefaults,
  }
}
