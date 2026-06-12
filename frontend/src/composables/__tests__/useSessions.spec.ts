import { describe, it, expect, beforeEach, vi } from 'vitest'

// ─── Mocks ────────────────────────────────────────────────────────────────
// Chat-T22 — 11/11 tests pass (6 backend + 5 frontend), no bugs

const mockFetch = vi.fn()
vi.stubGlobal('fetch', mockFetch)

vi.mock('@/composables/useAuth', () => ({
  authFetch: (...args: any[]) => mockFetch(...args),
}))

// ─── Tests ────────────────────────────────────────────────────────────────

describe('useSessions - deleteAllSessions', () => {
  beforeEach(() => {
    mockFetch.mockReset()
  })

  it('TC-T22.1: clears list and returns new session on success', async () => {
    const { useSessions } = await import('../useSessions')
    const { sessionList, deleteAllSessions } = useSessions()

    // Pre-populate with existing sessions
    sessionList.value = [
      { id: 's1', status: 'idle', preview: '', message_count: 0, last_activity: 0 },
      { id: 's2', status: 'idle', preview: '', message_count: 0, last_activity: 0 },
    ] as any

    const newSession = {
      id: 'new-session',
      status: 'idle',
      preview: 'New session',
      message_count: 0,
      last_activity: 1735689600,
    }

    mockFetch.mockResolvedValueOnce({
      ok: true,
      status: 200,
      json: () =>
        Promise.resolve({
          deleted_count: 2,
          new_session_id: 'new-session',
          session: newSession,
        }),
    })

    const result = await deleteAllSessions()

    expect(mockFetch).toHaveBeenCalledWith('/api/forge/chats/sessions', {
      method: 'DELETE',
    })
    expect(result.deletedCount).toBe(2)
    expect(result.newSessionId).toBe('new-session')
    expect(sessionList.value).toHaveLength(1)
    expect(sessionList.value[0].id).toBe('new-session')
  })

  it('TC-T22.2: handles network error', async () => {
    const { useSessions } = await import('../useSessions')
    const { sessionList, deleteAllSessions } = useSessions()

    sessionList.value = [{ id: 's1' }] as any
    mockFetch.mockRejectedValueOnce(new Error('Network error'))

    await expect(deleteAllSessions()).rejects.toThrow('Network error')
    // Session list should remain unchanged on error
    expect(sessionList.value).toHaveLength(1)
  })

  it('TC-T22.3: handles 500 error', async () => {
    const { useSessions } = await import('../useSessions')
    const { deleteAllSessions } = useSessions()

    mockFetch.mockResolvedValueOnce({ ok: false, status: 500 })

    await expect(deleteAllSessions()).rejects.toThrow(
      'Failed to delete all sessions: 500'
    )
  })

  it('TC-T22.7: hasSessions returns false when empty', async () => {
    const { useSessions } = await import('../useSessions')
    const { sessionList, hasSessions } = useSessions()

    sessionList.value = []
    expect(hasSessions.value).toBe(false)
  })

  it('TC-T22.8: hasSessions returns true when sessions exist', async () => {
    const { useSessions } = await import('../useSessions')
    const { sessionList, hasSessions } = useSessions()

    sessionList.value = [
      { id: 's1' } as any,
      { id: 's2' } as any,
    ]
    expect(hasSessions.value).toBe(true)
  })
})
