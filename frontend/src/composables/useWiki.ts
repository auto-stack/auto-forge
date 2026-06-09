import { ref } from 'vue'
import type { WikiPage, WikiPageMeta, TreeNode } from '@/types/wiki'
import { authFetch } from './useAuth'

const API_BASE = '/api/forge/wiki'
const RAW_BASE = '/api/forge/raw'

// ─── Singleton state ────────────────────────────────────────────────────────
const _pages = ref<WikiPageMeta[]>([])
const _activePage = ref<WikiPage | null>(null)
const _isLoading = ref(false)
const _error = ref<string | null>(null)
const _wikiTree = ref<TreeNode[]>([])
const _rawTree = ref<TreeNode[]>([])
const _uploadProgress = ref<number | null>(null)

export function useWiki() {
  const pages = _pages
  const activePage = _activePage
  const isLoading = _isLoading
  const error = _error
  const wikiTree = _wikiTree
  const rawTree = _rawTree
  const uploadProgress = _uploadProgress

  async function loadPages(project: string) {
    isLoading.value = true
    error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(project)}/pages`)
      if (!resp.ok) throw new Error(`Failed to load wiki pages: ${resp.status}`)
      const data = await resp.json()
      _pages.value = data.pages ?? []
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      isLoading.value = false
    }
  }

  async function loadPage(project: string, slug: string) {
    isLoading.value = true
    error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(project)}/page/${encodeURIComponent(slug)}`)
      if (!resp.ok) throw new Error(`Failed to load page: ${resp.status}`)
      const data = await resp.json()
      _activePage.value = data.page ?? null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      isLoading.value = false
    }
  }

  async function createPage(project: string, page: { slug: string; title: string; content: string; source_type: string; tags: string[] }) {
    error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(project)}/pages`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(page),
      })
      if (!resp.ok) {
        const body = await resp.text()
        throw new Error(`Failed to create page: ${resp.status} ${body}`)
      }
      await loadPages(project)
      await loadWikiTree(project)
      const data = await resp.json()
      _activePage.value = data.page ?? null
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function updatePage(project: string, slug: string, data: { content?: string; title?: string; tags?: string[] }) {
    error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(project)}/page/${encodeURIComponent(slug)}`, {
        method: 'PUT',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify(data),
      })
      if (!resp.ok) throw new Error(`Failed to update page: ${resp.status}`)
      await loadPages(project)
      await loadPage(project, slug)
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function deletePage(project: string, slug: string) {
    error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(project)}/page/${encodeURIComponent(slug)}`, {
        method: 'DELETE',
      })
      if (!resp.ok) throw new Error(`Failed to delete page: ${resp.status}`)
      if (_activePage.value?.slug === slug) _activePage.value = null
      await loadPages(project)
      await loadWikiTree(project)
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function searchWiki(project: string, query: string) {
    isLoading.value = true
    error.value = null
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(project)}/search`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ query }),
      })
      if (!resp.ok) throw new Error(`Search failed: ${resp.status}`)
      const data = await resp.json()
      _pages.value = (data.results ?? []).map((p: WikiPage) => ({
        slug: p.slug,
        title: p.title,
        source_type: p.source_type,
        tags: p.tags,
        version: p.version,
        updated_at: p.updated_at,
      }))
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    } finally {
      isLoading.value = false
    }
  }

  // ─── Tree Operations ────────────────────────────────────────────────────

  async function loadWikiTree(project: string) {
    try {
      const resp = await authFetch(`${API_BASE}/${encodeURIComponent(project)}/tree`)
      if (!resp.ok) throw new Error(`Failed to load wiki tree: ${resp.status}`)
      _wikiTree.value = await resp.json()
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function loadRawTree(project: string) {
    try {
      const resp = await authFetch(`${RAW_BASE}/${encodeURIComponent(project)}/tree`)
      if (!resp.ok) throw new Error(`Failed to load raw tree: ${resp.status}`)
      _rawTree.value = await resp.json()
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  function rawFileUrl(project: string, path: string): string {
    return `${RAW_BASE}/${encodeURIComponent(project)}/file/${encodeURIComponent(path)}`
  }

  async function uploadRawFiles(project: string, files: File[], prefix = '') {
    error.value = null
    const formData = new FormData()
    for (const file of files) {
      formData.append('files', file, file.name)
    }
    const query = prefix ? `?prefix=${encodeURIComponent(prefix)}` : ''
    const url = `${RAW_BASE}/${encodeURIComponent(project)}/upload${query}`

    return new Promise<void>((resolve, reject) => {
      const xhr = new XMLHttpRequest()
      xhr.upload.addEventListener('progress', (e) => {
        if (e.lengthComputable) {
          _uploadProgress.value = Math.round((e.loaded / e.total) * 100)
        }
      })
      xhr.addEventListener('load', () => {
        _uploadProgress.value = null
        if (xhr.status >= 200 && xhr.status < 300) {
          loadRawTree(project)
          resolve()
        } else {
          const msg = `Upload failed: ${xhr.status}`
          error.value = msg
          reject(new Error(msg))
        }
      })
      xhr.addEventListener('error', () => {
        _uploadProgress.value = null
        error.value = 'Upload failed'
        reject(new Error('Upload failed'))
      })
      xhr.open('POST', url)
      xhr.send(formData)
    })
  }

  async function deleteRawFile(project: string, path: string) {
    error.value = null
    try {
      const resp = await authFetch(`${RAW_BASE}/${encodeURIComponent(project)}/file/${encodeURIComponent(path)}`, {
        method: 'DELETE',
      })
      if (!resp.ok) throw new Error(`Failed to delete: ${resp.status}`)
      await loadRawTree(project)
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  async function createRawFolder(project: string, path: string) {
    error.value = null
    try {
      const resp = await authFetch(`${RAW_BASE}/${encodeURIComponent(project)}/mkdir`, {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ path }),
      })
      if (!resp.ok) throw new Error(`Failed to create folder: ${resp.status}`)
      await loadRawTree(project)
    } catch (e) {
      error.value = e instanceof Error ? e.message : String(e)
    }
  }

  return {
    pages, activePage, isLoading, error,
    wikiTree, rawTree, uploadProgress,
    loadPages, loadPage, createPage, updatePage, deletePage, searchWiki,
    loadWikiTree, loadRawTree, uploadRawFiles, deleteRawFile, createRawFolder, rawFileUrl,
  }
}
