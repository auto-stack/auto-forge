import { ref, computed } from 'vue'

const API_BASE = '/api/forge/config/api-sources'

export type ModelTier = 'min' | 'lite' | 'mid' | 'pro' | 'max'

export interface ModelDefinition {
  id: string
  name: string
  tier: ModelTier
}

export interface ApiSource {
  id: string
  name: string
  provider: 'anthropic' | 'openai' | 'local'
  api_key_env: string
  api_key_stored: string | null
  base_url: string | null
  is_available: boolean
  models: ModelDefinition[]
}

export interface ConnectionTestResult {
  success: boolean
  model: string | null
  error: string | null
  latency_ms: number | null
}

// Singleton state
const _sources = ref<ApiSource[]>([])
const _loading = ref(false)
const _error = ref<string | null>(null)
const _testResult = ref<ConnectionTestResult | null>(null)

export function useApiSources() {
  const sources = computed(() => _sources.value)
  const loading = computed(() => _loading.value)
  const error = computed(() => _error.value)
  const testResult = computed(() => _testResult.value)

  const hasSources = computed(() => _sources.value.length > 0)
  const availableSources = computed(() => _sources.value.filter(s => s.is_available))

  async function loadSources() {
    _loading.value = true
    _error.value = null
    try {
      const resp = await fetch(API_BASE)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      _sources.value = await resp.json()
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
    } finally {
      _loading.value = false
    }
  }

  async function createSource(source: ApiSource): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(API_BASE, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ source }),
      })
      if (!resp.ok) {
        const data = await resp.json().catch(() => ({}))
        throw new Error(data.error || `HTTP ${resp.status}`)
      }
      const created = await resp.json()
      _sources.value.push(created)
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function updateSource(id: string, source: ApiSource): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/${encodeURIComponent(id)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(source),
      })
      if (!resp.ok) {
        const data = await resp.json().catch(() => ({}))
        throw new Error(data.error || `HTTP ${resp.status}`)
      }
      const updated = await resp.json()
      const idx = _sources.value.findIndex(s => s.id === id)
      if (idx >= 0) _sources.value[idx] = updated
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function deleteSource(id: string): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/${encodeURIComponent(id)}`, {
        method: 'DELETE',
      })
      if (resp.status !== 204) throw new Error(`HTTP ${resp.status}`)
      _sources.value = _sources.value.filter(s => s.id !== id)
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function testConnection(id: string): Promise<ConnectionTestResult | null> {
    _testResult.value = null
    try {
      const resp = await fetch(`${API_BASE}/${encodeURIComponent(id)}/test`, {
        method: 'POST',
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      const result = await resp.json()
      _testResult.value = result
      return result
    } catch (e) {
      _testResult.value = { success: false, model: null, error: e instanceof Error ? e.message : String(e), latency_ms: null }
      return _testResult.value
    }
  }

  function getTierModels(sourceId: string, tier: ModelTier): ModelDefinition[] {
    const source = _sources.value.find(s => s.id === sourceId)
    if (!source) return []
    return source.models.filter(m => m.tier === tier)
  }

  async function scanSources(): Promise<ApiSource[]> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/scan`)
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      return await resp.json()
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return []
    }
  }

  async function importSources(sourceIds: string[]): Promise<boolean> {
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/import`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ source_ids: sourceIds }),
      })
      if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
      _sources.value = await resp.json()
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  function clearTestResult() {
    _testResult.value = null
  }

  function addDraft(source: ApiSource) {
    _sources.value.unshift(source)
  }

  function removeDraft(id: string) {
    _sources.value = _sources.value.filter(s => s.id !== id)
  }

  return {
    sources,
    loading,
    error,
    testResult,
    hasSources,
    availableSources,
    loadSources,
    createSource,
    updateSource,
    deleteSource,
    clearTestResult,
    addDraft,
    removeDraft,
    testConnection,
    getTierModels,
    scanSources,
    importSources,
  }
}
