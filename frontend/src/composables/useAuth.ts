import { ref, computed } from 'vue'

const API_BASE = '/api/auth'
const TOKEN_KEY = 'autoforge_jwt'
const USER_KEY = 'autoforge_user'

// ── Singleton state ────────────────────────────────────────────────────────
const _token = ref<string | null>(localStorage.getItem(TOKEN_KEY))
const _user = ref<{
  user_id: number
  username: string
  roles: string[]
  permissions: string[]
} | null>(null)
const _loading = ref(false)
const _error = ref<string | null>(null)

// Restore user from localStorage cache
try {
  const cached = localStorage.getItem(USER_KEY)
  if (cached) _user.value = JSON.parse(cached)
} catch {}

export function useAuth() {
  const token = _token
  const user = _user
  const loading = _loading
  const error = _error
  const isAuthenticated = computed(() => !!_token.value && !!_user.value)
  const isAdmin = computed(() => _user.value?.roles.includes('admin') ?? false)

  /** Login with username/password. Returns true on success. */
  async function login(username: string, password: string): Promise<boolean> {
    _loading.value = true
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/login`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      })
      if (!resp.ok) {
        const data = await resp.json().catch(() => ({ error: 'Login failed' }))
        _error.value = data.error || `Login failed (${resp.status})`
        return false
      }
      const data = await resp.json()
      _token.value = data.token
      _user.value = {
        user_id: data.user_id,
        username: data.username,
        roles: data.roles,
        permissions: [],
      }
      localStorage.setItem(TOKEN_KEY, data.token)
      localStorage.setItem(USER_KEY, JSON.stringify(_user.value))
      // Fetch full permissions
      await me()
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    } finally {
      _loading.value = false
    }
  }

  /** Register a new account. Auto-logins on success. */
  async function register(username: string, password: string): Promise<boolean> {
    _loading.value = true
    _error.value = null
    try {
      const resp = await fetch(`${API_BASE}/register`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ username, password }),
      })
      if (!resp.ok) {
        const data = await resp.json().catch(() => ({ error: 'Registration failed' }))
        _error.value = data.error || `Registration failed (${resp.status})`
        return false
      }
      const data = await resp.json()
      _token.value = data.token
      _user.value = {
        user_id: data.user_id,
        username: data.username,
        roles: data.roles,
        permissions: [],
      }
      localStorage.setItem(TOKEN_KEY, data.token)
      localStorage.setItem(USER_KEY, JSON.stringify(_user.value))
      await me()
      return true
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return false
    } finally {
      _loading.value = false
    }
  }

  /** Fetch current user info from /api/auth/me. */
  async function me(): Promise<void> {
    if (!_token.value) return
    try {
      const resp = await authFetch(`${API_BASE}/me`)
      if (!resp.ok) {
        logout()
        return
      }
      const data = await resp.json()
      _user.value = {
        user_id: data.user_id,
        username: data.username,
        roles: data.roles,
        permissions: data.permissions,
      }
      localStorage.setItem(USER_KEY, JSON.stringify(_user.value))
    } catch {
      logout()
    }
  }

  /** Clear auth state and redirect to login. */
  function logout() {
    _token.value = null
    _user.value = null
    _error.value = null
    localStorage.removeItem(TOKEN_KEY)
    localStorage.removeItem(USER_KEY)
  }

  /** Validate stored token on app mount. Returns true if valid. */
  async function validateStoredToken(): Promise<boolean> {
    if (!_token.value) return false
    try {
      await me()
      return !!_user.value
    } catch {
      return false
    }
  }

  return {
    token, user, loading, error,
    isAuthenticated, isAdmin,
    login, register, logout, me, validateStoredToken,
  }
}

// ── Auth-aware fetch wrapper ────────────────────────────────────────────────

/**
 * Drop-in replacement for `fetch()` that adds the Authorization header
 * and handles 401 responses by clearing auth state.
 */
export async function authFetch(
  input: string | URL | globalThis.Request,
  init?: RequestInit,
): Promise<Response> {
  const token = _token.value

  // Build a plain headers object preserving all original headers
  const mergedHeaders: Record<string, string> = {}
  if (init?.headers) {
    const h = init.headers
    if (h instanceof Headers) {
      h.forEach((v, k) => { mergedHeaders[k] = v })
    } else if (Array.isArray(h)) {
      for (const [k, v] of h) mergedHeaders[k] = v
    } else {
      Object.assign(mergedHeaders, h)
    }
  }
  if (token) {
    mergedHeaders['Authorization'] = `Bearer ${token}`
  }

  const resp = await fetch(input, { ...init, headers: mergedHeaders })

  if (resp.status === 401) {
    // Token expired or invalid — clear auth state
    _token.value = null
    _user.value = null
    localStorage.removeItem(TOKEN_KEY)
    localStorage.removeItem(USER_KEY)
  }

  return resp
}
