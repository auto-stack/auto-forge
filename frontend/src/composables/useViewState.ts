import { ref } from 'vue'

/**
 * Valid view identifiers matching App.vue tab IDs
 */
export type ViewId =
  | 'chats'
  | 'specs'
  | 'wiki'
  | 'agents'
  | 'agents-config'
  | 'professions'
  | 'skills'
  | 'apis'
  | 'explorer'

const VALID_VIEW_IDS: Set<ViewId> = new Set([
  'chats', 'specs', 'wiki', 'agents',
  'agents-config', 'professions', 'skills', 'apis', 'explorer',
])

const DEFAULT_VIEW: ViewId = 'chats'
const BASE_PATH = '/forge/'

/**
 * Validates if a value is a valid ViewId
 */
function isValidViewId(value: unknown): value is ViewId {
  return typeof value === 'string' && VALID_VIEW_IDS.has(value as ViewId)
}

/**
 * Parse the current URL pathname relative to the /forge/ base.
 * Returns the view and any remaining detail path segments.
 *
 * Examples:
 *   /forge/chats/abc-123       -> { view: 'chats', detailPath: 'abc-123' }
 *   /forge/specs/auth/goals/G1 -> { view: 'specs', detailPath: 'auth/goals/G1' }
 *   /forge/agents/run-123      -> { view: 'agents', detailPath: 'run-123' }
 *   /forge/                    -> { view: 'chats', detailPath: '' }
 */
function parseUrlPath(): { view: ViewId; detailPath: string } {
  const path = window.location.pathname
  let rest = path.startsWith(BASE_PATH) ? path.slice(BASE_PATH.length) : path.replace(/^\/+/, '')
  // Trim leading/trailing slashes
  rest = rest.replace(/^\/+/, '').replace(/\/+$/, '')

  if (!rest) {
    return { view: DEFAULT_VIEW, detailPath: '' }
  }

  const [rawView, ...detailParts] = rest.split('/')
  if (isValidViewId(rawView)) {
    return { view: rawView, detailPath: detailParts.join('/') }
  }

  return { view: DEFAULT_VIEW, detailPath: '' }
}

/**
 * Build a URL path for a given view and optional detail path.
 */
function buildPath(view: ViewId, detailPath?: string): string {
  if (!detailPath) {
    return `${BASE_PATH}${view}`
  }
  return `${BASE_PATH}${view}/${detailPath}`
}

/**
 * Push a new history entry without reloading the page.
 * Uses replaceState when only the detail path changes to avoid polluting history.
 */
function updateHistory(view: ViewId, detailPath: string, replace = false) {
  const path = buildPath(view, detailPath || undefined)
  if (window.location.pathname === path) return

  if (replace) {
    window.history.replaceState({}, '', path)
  } else {
    window.history.pushState({}, '', path)
  }
}

// Singleton state (matches existing composable pattern)
const _currentView = ref<ViewId>(DEFAULT_VIEW)
const _currentDetailPath = ref<string>('')
let _initialized = false

/**
 * View state persistence composable with URL routing.
 *
 * Provides:
 * - currentView: Reactive ref holding the current active view
 * - currentDetailPath: Reactive ref holding the remaining URL path after the view
 * - setView(view, detailPath?): Switch view and optionally set detail path, updating URL
 * - setDetailPath(detailPath): Update detail path only, updating URL
 * - restoreFromUrl(): Manually re-sync state from the URL (e.g. on popstate)
 *
 * The URL becomes the single source of truth for the top-level view and detail.
 */
export function useViewState() {
  if (!_initialized && typeof window !== 'undefined') {
    const parsed = parseUrlPath()
    _currentView.value = parsed.view
    _currentDetailPath.value = parsed.detailPath
    _initialized = true

    window.addEventListener('popstate', () => {
      const parsed = parseUrlPath()
      _currentView.value = parsed.view
      _currentDetailPath.value = parsed.detailPath
    })
  }

  function setView(view: ViewId, detailPath?: string) {
    if (!isValidViewId(view)) return
    const nextDetailPath = detailPath ?? ''
    _currentView.value = view
    _currentDetailPath.value = nextDetailPath
    updateHistory(view, nextDetailPath, false)
  }

  function setDetailPath(detailPath: string) {
    _currentDetailPath.value = detailPath
    updateHistory(_currentView.value, detailPath, true)
  }

  function restoreFromUrl() {
    const parsed = parseUrlPath()
    _currentView.value = parsed.view
    _currentDetailPath.value = parsed.detailPath
  }

  return {
    currentView: _currentView,
    currentDetailPath: _currentDetailPath,
    setView,
    setDetailPath,
    restoreFromUrl,
  }
}
