import { ref, watch } from 'vue'

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
  'agents-config', 'professions', 'skills', 'apis', 'explorer'
])

const STORAGE_KEY = 'autoforge-current-view'
const DEFAULT_VIEW: ViewId = 'chats'

/**
 * Validates if a value is a valid ViewId
 */
function isValidViewId(value: unknown): value is ViewId {
  return typeof value === 'string' && VALID_VIEW_IDS.has(value as ViewId)
}

/**
 * Loads current view from localStorage with validation
 * Falls back to DEFAULT_VIEW on any error
 */
function loadStoredView(): ViewId {
  try {
    const stored = localStorage.getItem(STORAGE_KEY)
    if (stored === null) return DEFAULT_VIEW
    if (isValidViewId(stored)) return stored
    // Invalid value, clear it and return default
    localStorage.removeItem(STORAGE_KEY)
    return DEFAULT_VIEW
  } catch (e) {
    // localStorage unavailable (private browsing, quota exceeded)
    console.warn('Failed to read view state from localStorage:', e)
    return DEFAULT_VIEW
  }
}

/**
 * Saves current view to localStorage
 * Silently fails if localStorage unavailable
 */
function saveStoredView(view: ViewId) {
  try {
    localStorage.setItem(STORAGE_KEY, view)
  } catch (e) {
    console.warn('Failed to persist view state to localStorage:', e)
  }
}

// Singleton state (matches existing composable pattern from UiSystem-A1)
const _currentView = ref<ViewId>(DEFAULT_VIEW)

/**
 * View state persistence composable
 * 
 * Provides:
 * - currentView: Reactive ref holding the current active view
 * - Automatic persistence to localStorage on changes
 * - Automatic restoration from localStorage on mount
 * 
 * @example
 * ```vue
 * <script setup>
 * const { currentView } = useViewState()
 * </script>
 * ```
 */
export function useViewState() {
  // Load from localStorage on first use
  if (_currentView.value === DEFAULT_VIEW) {
    _currentView.value = loadStoredView()
  }

  // Persist changes to localStorage (debounced implicitly by Vue batch)
  watch(_currentView, (newView) => {
    if (isValidViewId(newView)) {
      saveStoredView(newView)
    }
  })

  return {
    currentView: _currentView
  }
}
