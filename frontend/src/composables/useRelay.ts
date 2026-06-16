import { ref, computed } from 'vue'
import { useEventRouter, type SSEEvent } from './useEventRouter'
import { authFetch } from './useAuth'

const API_BASE = '/api/forge/relay'

// ─── Singleton state ────────────────────────────────────────────────────────
const _runs = ref<RunSummary[]>([])
const _currentRun = ref<RunState | null>(null)
const _professions = ref<ProfessionDto[]>([])
const _souls = ref<SoulDto[]>([])
const _loading = ref(false)
const _error = ref<string | null>(null)
const _liveLog = ref<Array<{ time: string; profession: string; action: string }>>([])
const _professionTokens = ref<Record<string, number>>({})
const _sessionLog = ref<SessionLogEntry[]>([])

export interface SessionLogEntry {
  id: string
  time: string
  profession_id: string
  step_id?: string
  type: 'text' | 'thinking' | 'tool_call' | 'tool_result' | 'tool' | 'complete' | 'error' | 'budget_warning' | 'budget_exceeded' | 'step_started' | 'step_completed' | 'gate_waiting' | 'run_completed' | 'run_failed'
  content: string
  tool_name?: string
  tool_id?: string
  arguments?: any
  result?: string
  remaining?: number
}

// ─── Types (mirroring Rust structs) ─────────────────────────────────────────

export interface RunSummary {
  run_id: string
  status: string
  current_step: number
  total_steps: number
  current_profession: string | null
  cumulative_tokens: number
  created_at: number
  updated_at: number
  title?: string
  task?: string
}

export interface RunEventDto {
  type: string
  timestamp?: number
  step_id?: string
  profession_id?: string
  handoff_summary?: string
  gate?: string
  decision?: string
  error?: string
  cumulative?: number
  step_tokens?: number
  text?: string
  tool_id?: string
  tool_name?: string
  arguments?: any
  result?: string
  message?: string
  remaining?: number
  thinking?: string
}

export interface RunState {
  run_id: string
  status: string
  current_step: number
  total_steps: number
  current_profession: string | null
  steps: StepState[]
  step_history: StepRecord[]
  cumulative_tokens: number
  budget_limit: number
  budget_remaining: number
  waiting_for_gate: GateState | null
  parallel_estimate: number
  savings: number
  savings_ratio: number
  events: RunEventDto[]
  title?: string
  current_step_started_at?: number
  profession_tokens?: Record<string, number>
}

export interface StepState {
  id: string
  profession_id: string
  status: string
  gate: string
}

export interface StepRecord {
  step_id: string
  profession_id: string
  started_at: number
  completed_at: number
  iteration: number
}

export interface GateState {
  step_id: string
  profession_id: string
  since: number
}

export interface ProfessionDto {
  id: string
  name: string
  phase: string
  owned_sections: string[]
  allowed_tools: string[]
}

export interface SoulDto {
  id: string
  name: string
}

export interface StartRunRequest {
  run_id?: string
  flow_id: string
  steps?: { id: string; profession_id: string; gate?: string }[]
  task?: string
}

// ─── Composable ─────────────────────────────────────────────────────────────

export function useRelay() {
  const runs = _runs
  const currentRun = _currentRun
  const professions = _professions
  const souls = _souls
  const loading = _loading
  const error = _error

  const hasActiveGate = computed(() => currentRun.value?.waiting_for_gate != null)
  const runProgress = computed(() => {
    if (!currentRun.value || currentRun.value.total_steps === 0) return 0
    return Math.round((currentRun.value.current_step / currentRun.value.total_steps) * 100)
  })
  const budgetUsedPercent = computed(() => {
    if (!currentRun.value || currentRun.value.budget_limit === 0) return 0
    const used = currentRun.value.budget_limit - currentRun.value.budget_remaining
    return Math.round((used / currentRun.value.budget_limit) * 100)
  })
  const liveLog = _liveLog
  const professionTokens = _professionTokens
  const sessionLog = _sessionLog

  async function loadProfessions() {
    try {
      const resp = await authFetch(`${API_BASE}/professions`)
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      const data = await resp.json()
      professions.value = data.professions
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function loadSouls() {
    try {
      const resp = await authFetch(`${API_BASE}/souls`)
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      const data = await resp.json()
      souls.value = data.souls
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function loadRuns(projectPath?: string) {
    try {
      const query = projectPath ? `?project_path=${encodeURIComponent(projectPath)}` : ''
      const resp = await authFetch(`${API_BASE}/runs${query}`)
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      const data = await resp.json()
      runs.value = data.sort((a: RunSummary, b: RunSummary) => b.updated_at - a.updated_at)
      // Clear stale currentRun if it's no longer in the list
      if (currentRun.value && !data.find((r: RunSummary) => r.run_id === currentRun.value!.run_id)) {
        currentRun.value = null
        _sessionLog.value = []
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  function formatTimestamp(ts: number): string {
    return new Date(ts * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  }

  function eventsToSessionLog(runId: string, events: RunEventDto[]): SessionLogEntry[] {
    const result: SessionLogEntry[] = []
    for (const ev of events) {
      const time = ev.timestamp ? formatTimestamp(ev.timestamp) : new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
      const prof = ev.profession_id || 'unknown'
      switch (ev.type) {
        case 'turn_delta':
          if (result.length > 0 && result[result.length - 1].type === 'text' && result[result.length - 1].profession_id === prof) {
            result[result.length - 1].content += ev.text || ''
          } else {
            result.push({ id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time, profession_id: prof, type: 'text', content: ev.text || '' })
          }
          break
        case 'turn_thinking':
          if (result.length > 0 && result[result.length - 1].type === 'thinking' && result[result.length - 1].profession_id === prof) {
            result[result.length - 1].content += ev.thinking || ''
          } else {
            result.push({ id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time, profession_id: prof, type: 'thinking', content: ev.thinking || '' })
          }
          break
        case 'turn_tool_call':
          result.push({ id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time, profession_id: prof, type: 'tool_call', content: '', tool_name: ev.tool_name, tool_id: ev.tool_id, arguments: ev.arguments })
          break
        case 'turn_tool_result': {
          const last = result[result.length - 1]
          if (last && last.type === 'tool_call' && last.tool_id === ev.tool_id) {
            // Merge into a single tool widget
            last.type = 'tool'
            last.result = ev.result || ''
          } else {
            result.push({ id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time, profession_id: prof, type: 'tool_result', content: ev.result || '', tool_id: ev.tool_id })
          }
          break
        }
        case 'turn_complete':
          result.push({ id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time, profession_id: prof, type: 'complete', content: 'Turn completed' })
          break
        case 'turn_error':
          result.push({ id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time, profession_id: prof, type: 'error', content: ev.message || 'Unknown error' })
          break
        case 'turn_budget_warning':
          result.push({ id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time, profession_id: prof, type: 'budget_warning', content: `Budget warning: ${ev.remaining} tokens remaining`, remaining: ev.remaining })
          break
        case 'turn_budget_exceeded':
          result.push({ id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`, time, profession_id: prof, type: 'budget_exceeded', content: 'Budget exceeded — turn stopped' })
          break
      }
    }
    return result
  }

  async function loadRun(runId: string) {
    try {
      const resp = await authFetch(`${API_BASE}/runs/${runId}`)
      if (!resp.ok) {
        if (resp.status === 404) {
          currentRun.value = null
          _sessionLog.value = []
        }
        throw new Error(`Failed: ${resp.status}`)
      }
      const data = await resp.json()
      currentRun.value = data
      // Populate session log from persisted events
      if (data.events && data.events.length > 0) {
        _sessionLog.value = eventsToSessionLog(runId, data.events)
      }
      // Populate profession tokens for cost breakdown when viewing historical runs
      if (data.profession_tokens) {
        _professionTokens.value = data.profession_tokens
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function startRun(req: StartRunRequest) {
    loading.value = true
    error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/runs`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(req),
      })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      const data = await resp.json()
      currentRun.value = data.state
      await loadRuns()
      return data.run_id as string
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      loading.value = false
    }
  }

  async function advanceRun(runId: string) {
    try {
      const resp = await authFetch(`${API_BASE}/runs/${runId}/advance`, { method: 'POST' })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      await loadRun(runId)
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function rerunRun(runId: string) {
    try {
      const resp = await authFetch(`${API_BASE}/runs/${runId}/rerun`, { method: 'POST' })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      await loadRun(runId)
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function resolveGate(runId: string, decision: 'approve' | 'reject' | 'edit', feedback?: string) {
    try {
      const body: any = { decision }
      if (feedback) body.feedback = feedback
      const resp = await authFetch(`${API_BASE}/runs/${runId}/gate`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      await loadRun(runId)
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function submitHandoff(runId: string, handoff: any) {
    try {
      const resp = await authFetch(`${API_BASE}/runs/${runId}/handoff`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ handoff }),
      })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      await loadRun(runId)
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function deleteRun(runId: string) {
    try {
      const resp = await authFetch(`${API_BASE}/runs/${runId}`, { method: 'DELETE' })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      if (currentRun.value?.run_id === runId) {
        currentRun.value = null
        _sessionLog.value = []
      }
      await loadRuns()
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function updateRunTitle(runId: string, title: string) {
    try {
      const resp = await authFetch(`${API_BASE}/runs/${runId}/title`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ title }),
      })
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      const updated = await resp.json()
      if (currentRun.value?.run_id === runId) {
        currentRun.value = updated
      }
      await loadRuns()
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      throw e
    }
  }

  // SSE for live updates
  function subscribeToRun(runId: string, onEvent?: (event: any) => void) {
    const eventRouter = useEventRouter()
    const es = new EventSource(`${API_BASE}/runs/${runId}/events`)
    es.onmessage = (event) => {
      try {
        const data = JSON.parse(event.data)
        if (onEvent) onEvent(data)
        // Route through event router for cross-view coordination
        const sseEvent: SSEEvent = {
          type: data.event_type || data.type,
          runId,
          payload: data,
        }
        eventRouter.handleEvent(sseEvent, 'relay')
        // Append to live log
        if (data.event_type === 'handoff_submitted') {
          _liveLog.value.push({
            time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
            profession: data.profession_id || data.from_profession || 'unknown',
            action: `Handoff to ${data.to_profession || 'next'}`,
          })
        }
        if (data.event_type === 'step_advanced') {
          _liveLog.value.push({
            time: new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' }),
            profession: data.profession_id || 'system',
            action: `Step advanced: ${data.step_id || ''}`,
          })
        }
        // Session log: turn events
        const time = new Date().toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
        const prof = data.payload?.profession_id || 'unknown'
        if (data.event_type === 'turn_delta') {
          const last = _sessionLog.value[_sessionLog.value.length - 1]
          if (last && last.type === 'text' && last.profession_id === prof) {
            last.content += data.payload.text || ''
          } else {
            _sessionLog.value.push({
              id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
              time,
              profession_id: prof,
              type: 'text',
              content: data.payload.text || '',
            })
          }
        }
        if (data.event_type === 'turn_thinking') {
          const last = _sessionLog.value[_sessionLog.value.length - 1]
          if (last && last.type === 'thinking' && last.profession_id === prof) {
            last.content += data.payload.thinking || ''
          } else {
            _sessionLog.value.push({
              id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
              time,
              profession_id: prof,
              type: 'thinking',
              content: data.payload.thinking || '',
            })
          }
        }
        if (data.event_type === 'turn_tool_call') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: prof,
            type: 'tool_call',
            content: '',
            tool_name: data.payload.tool_name,
            tool_id: data.payload.tool_id,
            arguments: data.payload.arguments,
          })
        }
        if (data.event_type === 'turn_tool_result') {
          const last = _sessionLog.value[_sessionLog.value.length - 1]
          if (last && last.type === 'tool_call' && last.tool_id === data.payload.tool_id) {
            last.type = 'tool'
            last.result = data.payload.result || ''
          } else {
            _sessionLog.value.push({
              id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
              time,
              profession_id: prof,
              type: 'tool_result',
              content: data.payload.result || '',
              tool_id: data.payload.tool_id,
            })
          }
        }
        if (data.event_type === 'turn_complete') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: prof,
            type: 'complete',
            content: 'Turn completed',
          })
        }
        if (data.event_type === 'turn_error') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: prof,
            type: 'error',
            content: data.payload.message || 'Unknown error',
          })
        }
        if (data.event_type === 'turn_budget_warning') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: prof,
            type: 'budget_warning',
            content: `Budget warning: ${data.payload.remaining} tokens remaining`,
            remaining: data.payload.remaining,
          })
        }
        if (data.event_type === 'turn_budget_exceeded') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: prof,
            type: 'budget_exceeded',
            content: 'Budget exceeded — turn stopped',
          })
        }
        // Step lifecycle events
        if (data.event_type === 'step_started') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: data.payload?.profession_id || 'system',
            type: 'step_started',
            content: `Step "${data.payload?.step_id || ''}" started`,
          })
        }
        if (data.event_type === 'step_completed') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: data.payload?.profession_id || 'system',
            type: 'step_completed',
            content: `Step "${data.payload?.step_id || ''}" completed`,
          })
        }
        if (data.event_type === 'gate_waiting') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: data.payload?.profession_id || 'system',
            type: 'gate_waiting',
            content: `Waiting for human gate approval`,
          })
        }
        if (data.event_type === 'run_completed') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: 'system',
            type: 'run_completed',
            content: 'Run completed successfully',
          })
        }
        if (data.event_type === 'run_failed') {
          _sessionLog.value.push({
            id: `${runId}-${Date.now()}-${Math.random().toString(36).slice(2, 7)}`,
            time,
            profession_id: 'system',
            type: 'run_failed',
            content: data.payload?.error || 'Run failed',
          })
        }
        // Track per-profession tokens (best-effort from event data)
        if (data.tokens_used && data.profession_id) {
          const prev = _professionTokens.value[data.profession_id] || 0
          _professionTokens.value[data.profession_id] = prev + (data.tokens_used as number)
        }
        // Auto-refresh run state on relevant events
        if (['run_started', 'step_started', 'step_advanced', 'handoff_submitted', 'gate_resolved', 'run_title_updated'].includes(data.event_type)) {
          loadRun(runId)
        }
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
    runs,
    currentRun,
    professions,
    souls,
    loading,
    error,
    hasActiveGate,
    runProgress,
    budgetUsedPercent,
    liveLog,
    professionTokens,
    loadProfessions,
    loadSouls,
    loadRuns,
    loadRun,
    startRun,
    advanceRun,
    rerunRun,
    resolveGate,
    submitHandoff,
    subscribeToRun,
    deleteRun,
    updateRunTitle,
    sessionLog,
  }
}
