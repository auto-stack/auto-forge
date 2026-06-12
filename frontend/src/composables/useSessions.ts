import { ref, computed } from 'vue'
import type { ForgeSession, ForgeSessionSummary } from '@/types/forge'
import { authFetch } from './useAuth'

const API_BASE = '/api/forge/chats'

// ─── Singleton state ───────────────────────────────────────────────────────
const _sessionList = ref<ForgeSessionSummary[]>([])
const _isLoading = ref(false)
const _error = ref<string | null>(null)

export function useSessions() {
  const sessionList = _sessionList
  const isLoading = _isLoading
  const error = _error

  /** Fetch all sessions from the server */
  async function loadSessionList() {
    try {
      _isLoading.value = true
      const resp = await authFetch(`${API_BASE}/sessions`)
      if (!resp.ok) throw new Error(`Failed to load sessions: ${resp.status}`)
      const data: ForgeSessionSummary[] = await resp.json()
      _sessionList.value = data
      _error.value = null
      return data
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      throw e
    } finally {
      _isLoading.value = false
    }
  }

  /** Delete a single session by ID */
  async function deleteSession(sessionId: string) {
    try {
      _isLoading.value = true
      const resp = await authFetch(`${API_BASE}/session/${sessionId}`, {
        method: 'DELETE',
      })
      if (!resp.ok) throw new Error(`Failed to delete session: ${resp.status}`)
      
      // Remove from local list
      _sessionList.value = _sessionList.value.filter(s => s.id !== sessionId)
      _error.value = null
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      throw e
    } finally {
      _isLoading.value = false
    }
  }

  /** Delete all sessions and create a new blank session */
  async function deleteAllSessions(): Promise<{ deletedCount: number; newSessionId: string; session: ForgeSession }> {
    try {
      _isLoading.value = true
      const resp = await authFetch(`${API_BASE}/sessions`, {
        method: 'DELETE',
      })
      
      if (!resp.ok) {
        throw new Error(`Failed to delete all sessions: ${resp.status}`)
      }
      
      const data = await resp.json()
      const { deleted_count, new_session_id, session } = data
      
      // Update local session list with the new session
      _sessionList.value = [session]
      _error.value = null
      
      return {
        deletedCount: deleted_count,
        newSessionId: new_session_id,
        session,
      }
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      throw e
    } finally {
      _isLoading.value = false
    }
  }

  /** Get count of sessions */
  const sessionCount = computed(() => _sessionList.value.length)

  /** Check if there are any sessions to delete */
  const hasSessions = computed(() => _sessionList.value.length > 0)

  return {
    sessionList,
    isLoading,
    error,
    sessionCount,
    hasSessions,
    loadSessionList,
    deleteSession,
    deleteAllSessions,
  }
}
