import { ref, computed } from 'vue'

const API_BASE = '/api/forge/config/agents'

export interface AgentConfigDto {
  id: string
  name: string
  profession_id: string
  soul_id: string
  api_source_id: string
  model_id: string
  model_tier: 'min' | 'lite' | 'mid' | 'pro' | 'max'
  is_default: boolean
  temperature: number
  max_tokens: number
  reasoning_budget: number | null
  thinking_enabled: boolean
  thinking_budget: number | null
  avatar_url?: string
  equipped_skills?: string[]
}

// Singleton state
const _configs = ref<AgentConfigDto[]>([])
const _loading = ref(false)
const _error = ref<string | null>(null)

export function useAgentConfigs() {
  const configs = computed(() => _configs.value)
  const loading = computed(() => _loading.value)
  const error = computed(() => _error.value)

  const defaultConfigs = computed(() => _configs.value.filter(c => c.is_default))
  const customConfigs = computed(() => _configs.value.filter(c => !c.is_default))

  function getByProfession(professionId: string): AgentConfigDto | undefined {
    return _configs.value.find(c => c.profession_id === professionId && c.is_default)
  }

  function getById(id: string): AgentConfigDto | undefined {
    return _configs.value.find(c => c.id === id)
  }

  async function loadConfigs() {
    _loading.value = true
    _error.value = null
    try {
      const resp = await fetch(API_BASE)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      _configs.value = await resp.json()
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
    } finally {
      _loading.value = false
    }
  }

  async function createConfig(config: AgentConfigDto): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(API_BASE, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ config }),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const created = await resp.json()
      _configs.value.push(created)
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function updateConfig(id: string, config: AgentConfigDto): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/${encodeURIComponent(id)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(config),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const updated = await resp.json()
      const idx = _configs.value.findIndex(c => c.id === id)
      if (idx >= 0) _configs.value[idx] = updated
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function deleteConfig(id: string): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/${encodeURIComponent(id)}`, { method: 'DELETE' })
      if (resp.status === 403) {
        _error.value = 'Cannot delete default agents'
        return false
      }
      if (resp.status !== 204) throw new Error(`HTTP ${resp.status}`)
      _configs.value = _configs.value.filter(c => c.id !== id)
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
      _configs.value = await resp.json()
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  return {
    configs,
    loading,
    error,
    defaultConfigs,
    customConfigs,
    getByProfession,
    getById,
    loadConfigs,
    createConfig,
    updateConfig,
    deleteConfig,
    resetDefaults,
  }
}
