import { ref, computed } from 'vue'
import type { ForgeMessage, ForgeSession, ForgeSessionSummary, ForgeStreamEvent, ErrandState } from '@/types/forge'
import type { ToolCallInfo } from '@/types/tool'
import { useEventRouter, type SSEEvent } from './useEventRouter'
import { authFetch } from './useAuth'
import { useViewState } from './useViewState'

const API_BASE = '/api/forge/chats'
const STORAGE_KEY = 'autoforge_session_id'

// ─── Singleton state: persists across component instances ───────────────────
const _session = ref<ForgeSession | null>(null)
const _messages = ref<ForgeMessage[]>([])
const _isLoading = ref(false)
const _error = ref<string | null>(null)
const _sessionList = ref<ForgeSessionSummary[]>([])
const _resuming = ref(false)
const _errands = ref<Record<string, ErrandState>>({})
const _relayRuns = ref<Record<string, import('@/types/forge').RelayRunState>>({})
const _taskPlans = ref<Record<string, import('@/types/forge').TaskPlanState>>({})

export function useForge() {
  const session = _session
  const messages = _messages
  const isLoading = _isLoading
  const error = _error
  const sessionList = _sessionList
  const errands = _errands
  const taskPlans = _taskPlans

  // URL view state: keep /forge/chats/{sessionId} in sync
  const viewState = typeof window !== 'undefined' ? useViewState() : null

  const sessionId = computed(() => session.value?.id ?? null)

  function syncSessionUrl(sid: string | null) {
    if (!viewState) return
    if (sid) {
      viewState.setDetailPath(sid)
    } else {
      viewState.setDetailPath('')
    }
  }

  const sessionStatus = computed(() => session.value?.status ?? 'idle')
  const sessionPhase = computed(() => session.value?.phase ?? 'intake')
  const needsApproval = computed(() => sessionStatus.value === 'waiting_approval' && sessionPhase.value === 'spec_review')
  const pendingSpecChanges = computed(() => session.value?.pending_spec_changes ?? [])

  /** Create a brand-new Forge session */
  async function createSession(notebookSid?: string, projectPath?: string) {
    try {
      const resp = await authFetch(`${API_BASE}/session`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ notebook_sid: notebookSid, project_path: projectPath }),
      })
      if (!resp.ok) throw new Error(`Failed to create session: ${resp.status}`)
      const data: ForgeSession = await resp.json()
      session.value = data
      messages.value = data.messages
      error.value = null
      localStorage.setItem(STORAGE_KEY, data.id)
      syncSessionUrl(data.id)
      await loadSessionList()
      return data.id
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return null
    }
  }

  /** Restore an existing session by ID (from localStorage or URL) */
  async function restoreSession(sid: string) {
    try {
      const resp = await authFetch(`${API_BASE}/session/${sid}`)
      if (!resp.ok) throw new Error(`Session not found: ${resp.status}`)
      const data: ForgeSession | null = await resp.json()
      if (!data) throw new Error('Session returned null')

      session.value = data
      messages.value = data.messages
      error.value = null
      localStorage.setItem(STORAGE_KEY, data.id)
      syncSessionUrl(data.id)
      return data.id
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      localStorage.removeItem(STORAGE_KEY)
      return null
    }
  }

  /** Switch to a different existing session */
  async function switchSession(sid: string) {
    if (sessionId.value === sid) return sid
    const restored = await restoreSession(sid)
    if (restored) {
      await loadSessionList()
    }
    return restored
  }

  /** Start fresh: clear local state and storage, then create a new session */
  async function clearSession(projectPath?: string) {
    session.value = null
    messages.value = []
    error.value = null
    localStorage.removeItem(STORAGE_KEY)
    syncSessionUrl(null)
    await createSession(undefined, projectPath)
  }

  /** Attempt to resume on app load:
   *  1. Check URL detail path for a requested session ID
   *  2. Check localStorage for a previous session ID
   *  3. Try to restore it from the server
   *  4. Reuse an existing idle session if available
   *  5. Only create a new session as last resort
   */
  async function resume(projectPath?: string) {
    if (_resuming.value) return _session.value?.id ?? null
    _resuming.value = true
    try {
      // Prefer explicit URL path, e.g. /forge/chats/{sessionId}
      const urlSessionId = viewState?.currentDetailPath.value ?? ''
      if (urlSessionId) {
        const restored = await restoreSession(urlSessionId)
        if (restored) return restored
        // Invalid URL session — clear it so we don't keep retrying
        syncSessionUrl(null)
      }

      const stored = localStorage.getItem(STORAGE_KEY)
      if (stored) {
        const restored = await restoreSession(stored)
        if (restored) return restored
      }

      // No valid stored session — try to reuse an existing idle one
      await loadSessionList()
      const idle = sessionList.value
        .filter((s) => s.status === 'idle')
        .sort((a, b) => b.last_activity - a.last_activity)[0]
      if (idle) {
        return await restoreSession(idle.id)
      }

      return await createSession(undefined, projectPath)
    } finally {
      _resuming.value = false
    }
  }

  /** Fetch the list of all sessions from the server */
  async function loadSessionList() {
    try {
      const resp = await authFetch(`${API_BASE}/sessions`)
      if (resp.ok) {
        const data: ForgeSessionSummary[] = await resp.json()
        sessionList.value = data
      }
    } catch {
      // ignore
    }
  }

  async function sendMessage(content: string, professionId?: string) {
    if (!sessionId.value || isLoading.value) return

    const userMsg: ForgeMessage = {
      id: `u-${Date.now()}`,
      role: 'user',
      content,
      timestamp: Date.now(),
      profession_id: professionId,
    }
    messages.value.push(userMsg)
    isLoading.value = true
    error.value = null

    try {
      const body: Record<string, string> = { content }
      if (professionId) body.profession_id = professionId
      const resp = await authFetch(`${API_BASE}/${sessionId.value}/message`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(body),
      })
      if (!resp.ok) throw new Error(`Failed to send message: ${resp.status}`)

      await streamResponse()
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      isLoading.value = false
    }
  }

  async function streamResponse() {
    if (!sessionId.value) return

    // Current assistant message for this turn — reset on each turn_start
    let assistantMsg: ForgeMessage | null = null

    function newAssistantMsg(professionId?: string): ForgeMessage {
      // Reuse the current assistant message if it's completely empty (no content, no tool calls).
      // This prevents creating ghost messages for turns that only contained tool calls.
      if (assistantMsg && assistantMsg.content === '' && (!assistantMsg.tool_calls || assistantMsg.tool_calls.length === 0)) {
        assistantMsg.profession_id = professionId ?? assistantMsg.profession_id
        return assistantMsg
      }
      const msg: ForgeMessage = {
        id: `a-${Date.now()}-${Math.random().toString(36).slice(2, 6)}`,
        role: 'assistant',
        content: '',
        thinking: '',
        timestamp: Date.now(),
        tool_calls: [],
        profession_id: professionId ?? undefined,
      }
      messages.value.push(msg)
      assistantMsg = msg
      return msg
    }

    function ensureAssistantMsg(): ForgeMessage {
      if (!assistantMsg) {
        return newAssistantMsg()
      }
      return assistantMsg
    }

    try {
      const eventSource = new EventSource(`${API_BASE}/${sessionId.value}/stream`)

      const eventRouter = useEventRouter()

      eventSource.onmessage = (event) => {
        try {
          const data: ForgeStreamEvent = JSON.parse(event.data)

          // Route cross-cutting events through event router
          if (data.type === 'gate_reached' || data.type === 'run_completed') {
            const sseEvent: SSEEvent = {
              type: data.type,
              runId: data.run_id || sessionId.value || 'unknown',
              payload: data as unknown as Record<string, unknown>,
            }
            eventRouter.handleEvent(sseEvent, 'chat')
          }

          if (data.type === 'turn_start') {
            // New turn = new assistant message bubble
            newAssistantMsg(data.profession_id)
          } else if (data.type === 'delta' && data.text) {
            const msg = ensureAssistantMsg()
            msg.content += data.text
          } else if (data.type === 'thinking' && data.thinking) {
            const msg = ensureAssistantMsg()
            msg.thinking = (msg.thinking || '') + data.thinking
          } else if (data.type === 'tool_call') {
            const msg = ensureAssistantMsg()
            const call: ToolCallInfo = {
              id: data.id ?? `tc-${Date.now()}`,
              name: data.name ?? 'unknown',
              arguments: (data.arguments as Record<string, unknown>) ?? {},
              status: 'running',
            }
            msg.tool_calls = msg.tool_calls ?? []
            msg.tool_calls.push(call)
          } else if (data.type === 'tool_result') {
            if (assistantMsg) {
              const call = assistantMsg.tool_calls?.find((c) => c.id === data.id)
              if (call) {
                call.result = data.result ?? ''
                call.status = 'success'
              }
            }
          } else if (data.type === 'agent_handoff') {
            if (session.value) {
              session.value.active_profession = data.to_profession
            }
          } else if (data.type === 'phase_change' && data.phase) {
            if (session.value) {
              session.value.phase = data.phase as ForgeSession['phase']
            }
          } else if (data.type === 'done') {
            eventSource.close()
            isLoading.value = false
            // Reset active_profession back to assistant after task completes
            if (session.value) {
              session.value.active_profession = undefined
            }
            loadSessionList()
          } else if (data.type === 'error') {
            eventSource.close()
            const msg = ensureAssistantMsg()
            msg.content += `\n\n[Error: ${data.message}]`
            isLoading.value = false
          } else if (data.type === 'errand_start' && data.errand_id) {
            _errands.value[data.errand_id] = {
              errand_id: data.errand_id,
              profession_id: data.profession_id || 'gofer',
              tool_call_id: data.tool_call_id || '',
              task: data.task || '',
              content: '',
              tool_calls: [],
              status: 'running',
            }
          } else if (data.type === 'errand_delta' && data.errand_id && data.text) {
            const e = _errands.value[data.errand_id]
            if (e) e.content += data.text
          } else if (data.type === 'errand_tool_call' && data.errand_id) {
            const e = _errands.value[data.errand_id]
            if (e) {
              e.tool_calls.push({
                id: data.id || `etc-${Date.now()}`,
                name: data.name || 'unknown',
                arguments: (data.arguments as Record<string, unknown>) ?? {},
                status: 'running',
              })
            }
          } else if (data.type === 'errand_tool_result' && data.errand_id) {
            const e = _errands.value[data.errand_id]
            if (e) {
              const tc = e.tool_calls.find((c) => c.id === data.id)
              if (tc) {
                tc.result = data.result ?? ''
                tc.status = 'success'
              }
            }
          } else if (data.type === 'errand_complete' && data.errand_id) {
            const e = _errands.value[data.errand_id]
            if (e) {
              e.status = (data.status as ErrandState['status']) || 'completed'
              e.result = data.result || e.content
              e.token_usage = data.token_usage
            }
          } else if (data.type === 'relay_spawned' && data.run_id) {
            _relayRuns.value[data.run_id] = {
              run_id: data.run_id,
              flow_id: data.flow_id || 'standard',
              status: 'started',
              steps: [],
            }
          } else if (data.type === 'relay_update' && data.run_id) {
            const r = _relayRuns.value[data.run_id]
            if (r) {
              r.steps.push({ step_id: data.step_id || '', profession_id: data.profession_id || '' })
              r.status = 'running'
            }
          } else if (data.type === 'relay_gate_waiting' && data.run_id) {
            const r = _relayRuns.value[data.run_id]
            if (r) {
              r.status = 'gate_waiting'
            }
          } else if (data.type === 'relay_complete' && data.run_id) {
            const r = _relayRuns.value[data.run_id]
            if (r) {
              r.status = data.status === 'failed' ? 'failed' : 'completed'
              r.summary = data.summary || ''
              r.tokens_used = data.tokens_used || 0
            }
          } else if (data.type === 'task_plan_spawned' && data.instance_id) {
            _taskPlans.value[data.instance_id] = {
              instance_id: data.instance_id,
              task_plan_id: data.task_plan_id || '',
              status: 'started',
              phases: [],
            }
          }
        } catch {
          const msg = ensureAssistantMsg()
          msg.content += event.data
        }
      }

      eventSource.onerror = () => {
        eventSource.close()
        isLoading.value = false
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      isLoading.value = false
    }
  }

  async function loadHistory() {
    if (!sessionId.value) return
    try {
      const resp = await authFetch(`${API_BASE}/${sessionId.value}/history`)
      if (resp.ok) {
        const data: ForgeMessage[] = await resp.json()
        if (data.length > 0) messages.value = data
      }
    } catch {
      // Ignore history load errors
    }
  }

  async function approveSpec(editedSpecs?: Record<string, string>) {
    if (!sessionId.value) return
    try {
      const resp = await authFetch(`${API_BASE}/${sessionId.value}/approve`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ edited_specs: editedSpecs ?? {} }),
      })
      if (!resp.ok) throw new Error(`Failed to approve: ${resp.status}`)
      const data = await resp.json()
      if (session.value) {
        session.value.phase = data.phase
        session.value.status = 'idle'
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function rejectSpec() {
    if (!sessionId.value) return
    try {
      const resp = await authFetch(`${API_BASE}/${sessionId.value}/reject`, {
        method: 'POST',
      })
      if (!resp.ok) throw new Error(`Failed to reject: ${resp.status}`)
      const data = await resp.json()
      if (session.value) {
        session.value.phase = data.phase
        session.value.status = 'idle'
      }
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function renameSession(sid: string, name: string) {
    try {
      const resp = await authFetch(`${API_BASE}/session/${sid}`, {
        method: 'PATCH',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ name }),
      })
      if (!resp.ok) throw new Error(`Failed to rename session: ${resp.status}`)
      await loadSessionList()
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function deleteSession(sid: string) {
    try {
      const resp = await authFetch(`${API_BASE}/session/${sid}`, {
        method: 'DELETE',
      })
      if (!resp.ok) throw new Error(`Failed to delete session: ${resp.status}`)
      // If we deleted the current session, clear local state
      if (sessionId.value === sid) {
        session.value = null
        messages.value = []
        localStorage.removeItem(STORAGE_KEY)
      }
      await loadSessionList()
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  async function deleteAllSessions() {
    try {
      const resp = await authFetch(`${API_BASE}/sessions`, {
        method: 'DELETE',
      })
      if (!resp.ok) throw new Error(`Failed to delete all sessions: ${resp.status}`)
      session.value = null
      messages.value = []
      localStorage.removeItem(STORAGE_KEY)
      await loadSessionList()
      return true
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
      return false
    }
  }

  return {
    session,
    messages,
    isLoading,
    error,
    sessionList,
    sessionId,
    sessionStatus,
    sessionPhase,
    needsApproval,
    pendingSpecChanges,
    createSession,
    restoreSession,
    switchSession,
    clearSession,
    resume,
    loadSessionList,
    sendMessage,
    loadHistory,
    streamResponse,
    approveSpec,
    rejectSpec,
    renameSession,
    deleteSession,
    deleteAllSessions,
    errands,
    relayRuns: _relayRuns,
    taskPlans: _taskPlans,
  }
}
