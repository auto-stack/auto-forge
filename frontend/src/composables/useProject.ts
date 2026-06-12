import { ref, computed } from 'vue'
import { authFetch } from './useAuth'

export interface ProjectInfo {
  path: string
  name: string
  specs_dir: string
  has_specs: boolean
  is_open: boolean
  is_empty?: boolean
}

export interface RecentProject {
  path: string
  name: string
  last_opened: number
}

export interface BrowseEntry {
  name: string
  path: string
  is_dir: boolean
}

const API_BASE = '/api/forge/project'

// Singleton state
const _projectInfo = ref<ProjectInfo | null>(null)
const _isLoading = ref(false)
const _error = ref<string | null>(null)
const _recentProjects = ref<RecentProject[]>([])

export function useProject() {
  const projectInfo = _projectInfo
  const isLoading = _isLoading
  const error = _error
  const recentProjects = _recentProjects

  const isOpen = computed(() => _projectInfo.value?.is_open ?? false)
  const projectName = computed(() => _projectInfo.value?.name ?? null)
  const projectPath = computed(() => _projectInfo.value?.path ?? null)

  async function fetchStatus() {
    try {
      const resp = await authFetch(`${API_BASE}/status`)
      if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
      const data: ProjectInfo = await resp.json()
      _projectInfo.value = data
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function openProject(path: string): Promise<ProjectInfo | null> {
    _isLoading.value = true
    _error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/open`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path }),
      })
      if (!resp.ok) {
        const msg = await resp.text()
        throw new Error(msg || `Failed to open project: ${resp.status}`)
      }
      const data: ProjectInfo = await resp.json()
      _projectInfo.value = data
      localStorage.setItem('autoforge-last-project', path)
      return data
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
      return null
    } finally {
      _isLoading.value = false
    }
  }

  async function closeProject() {
    try {
      await authFetch(`${API_BASE}/close`, { method: 'POST' })
      _projectInfo.value = null
    } catch (e) {
      _error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function fetchRecentProjects() {
    try {
      const resp = await authFetch(`${API_BASE}/recent`)
      if (!resp.ok) return
      const data: RecentProject[] = await resp.json()
      _recentProjects.value = data
    } catch { /* ignore */ }
  }

  async function browseDirectory(path: string): Promise<BrowseEntry[]> {
    const resp = await authFetch(`${API_BASE}/browse?path=${encodeURIComponent(path)}`)
    if (!resp.ok) throw new Error(`Browse failed: ${resp.status}`)
    const data = await resp.json()
    return data.children ?? []
  }

  return {
    projectInfo, isLoading, error, recentProjects,
    isOpen, projectName, projectPath,
    fetchStatus, openProject, closeProject,
    fetchRecentProjects, browseDirectory,
  }
}
