import { ref } from 'vue'
import { authFetch } from './useAuth'

const API_BASE = '/api/forge/relay/task_plans'

export interface TaskPlanSummary {
  id: string
  source: string
  phase_count: number
  run_count: number
}

export interface RunRef {
  name: string
  flow_id: string
  input?: string
  input_from: string[]
  context?: string
  mode_override?: string
}

export interface Phase {
  name: string
  mode: string
  depends_on: string[]
  runs: RunRef[]
}

export interface TaskPlanDetail {
  id: string
  version: number
  title?: string
  description?: string
  default_mode: string
  phases: Phase[]
}

export interface TaskPlanRunItem {
  run_id: string
  task_plan_id?: string
  phase_name?: string
  task_run_name?: string
  status: string
}

export interface TaskPlanRunResponse {
  instance_id: string
  task_plan_id: string
  status: string
}

export interface TaskPlanValidationResult {
  valid: boolean
  error?: string
}

const _plans = ref<TaskPlanSummary[]>([])
const _currentPlan = ref<TaskPlanDetail | null>(null)
const _runs = ref<TaskPlanRunItem[]>([])
const _loading = ref(false)
const _error = ref<string | null>(null)

export function useTaskPlan() {
  async function loadTaskPlans() {
    _loading.value = true
    _error.value = null
    try {
      const resp = await authFetch(API_BASE)
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      _plans.value = await resp.json()
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
    } finally {
      _loading.value = false
    }
  }

  async function getTaskPlan(id: string) {
    _loading.value = true
    _error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${id}`)
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      _currentPlan.value = await resp.json()
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      _currentPlan.value = null
    } finally {
      _loading.value = false
    }
  }

  async function startTaskPlanRun(id: string, initialInput: string, mode?: string): Promise<TaskPlanRunResponse | null> {
    _loading.value = true
    _error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${id}/runs`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ initial_input: initialInput, mode }),
      })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      const data: TaskPlanRunResponse = await resp.json()
      await loadTaskPlanRuns()
      return data
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      _loading.value = false
    }
  }

  async function loadTaskPlanRuns() {
    try {
      const resp = await authFetch(`${API_BASE}/runs`)
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      _runs.value = await resp.json()
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function registerTaskPlan(atom: string, filePath?: string): Promise<TaskPlanDetail | null> {
    _loading.value = true
    _error.value = null
    try {
      const resp = await authFetch(API_BASE, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ atom, file_path: filePath }),
      })
      if (!resp.ok) {
        const err = await resp.json()
        throw new Error(err.error || `Failed: ${resp.status}`)
      }
      const data: TaskPlanDetail = await resp.json()
      await loadTaskPlans()
      return data
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      _loading.value = false
    }
  }

  async function validateTaskPlan(atom: string): Promise<TaskPlanValidationResult> {
    try {
      const resp = await authFetch(`${API_BASE}/validate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ atom }),
      })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      return await resp.json()
    } catch (e) {
      return { valid: false, error: e instanceof Error ? e.message : String(e) }
    }
  }

  function subscribeToTaskPlan(instanceId: string, onEvent?: (event: any) => void) {
    const es = new EventSource(`${API_BASE}/${instanceId}/events`)
    es.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data)
        if (onEvent) onEvent(data)
      } catch {
        // ignore parse errors
      }
    }
    es.onerror = () => {
      // Will auto-reconnect or close
    }
    return () => es.close()
  }

  return {
    plans: _plans,
    currentPlan: _currentPlan,
    runs: _runs,
    loading: _loading,
    error: _error,
    loadTaskPlans,
    getTaskPlan,
    startTaskPlanRun,
    loadTaskPlanRuns,
    registerTaskPlan,
    validateTaskPlan,
    subscribeToTaskPlan,
  }
}
