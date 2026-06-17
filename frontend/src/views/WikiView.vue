<template>
  <div class="wiki-view">
    <div class="wiki-body">
      <!-- Sidebar -->
      <div class="wiki-nav" :class="{ collapsed: sidebarCollapsed }">
        <div class="wiki-nav-header">
          <span class="wiki-nav-title">{{ t('wiki.title') }}</span>
          <div class="wiki-nav-actions">
            <button class="nav-icon-btn" @click="startCreate" :title="t('wiki.newPage')">
              <Plus :size="14" />
            </button>
            <button class="nav-icon-btn" @click="sidebarCollapsed = !sidebarCollapsed" :title="t('wiki.toggleSidebar')">
              <PanelLeft :size="14" />
            </button>
          </div>
        </div>

        <div class="wiki-nav-list">
          <!-- Raw section -->
          <div class="tree-section">
            <div class="tree-section-header" @click="rawExpanded = !rawExpanded">
              <component :is="rawExpanded ? ChevronDown : ChevronRight" :size="12" />
              <FolderInput :size="13" />
              <span class="tree-section-title">{{ t('wiki.raw') }}</span>
              <button class="section-btn" @click.stop="startNewFolder('raw')" :title="t('wiki.newFolder')">
                <FolderPlus :size="11" />
              </button>
            </div>
            <div v-if="rawExpanded" class="tree-section-body">
              <TreeView
                v-for="node in filteredRawTree"
                :key="node.path"
                :node="node"
                :active-path="activeRawPath"
                @select="selectRawNode"
              />
              <DropZone :upload-progress="uploadProgress" @drop="handleRawDrop" />
            </div>
          </div>

          <!-- Wiki section -->
          <div class="tree-section">
            <div class="tree-section-header" @click="wikiExpanded = !wikiExpanded">
              <component :is="wikiExpanded ? ChevronDown : ChevronRight" :size="12" />
              <BookOpen :size="13" />
              <span class="tree-section-title">{{ t('wiki.title') }}</span>
            </div>
            <div v-if="wikiExpanded" class="tree-section-body">
              <TreeView
                v-for="node in filteredWikiTree"
                :key="node.path"
                :node="node"
                :active-path="activeWikiPath"
                @select="selectWikiNode"
              />
              <div v-if="wikiTree.length === 0" class="tree-empty">
                <FileText :size="14" />
                <span>{{ t('wiki.noPages') }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- Content pane -->
      <div class="wiki-content">
        <div class="content-header">
          <div class="header-left">
            <button v-if="sidebarCollapsed" class="nav-icon-btn" @click="sidebarCollapsed = false" :title="t('wiki.showSidebar')">
              <PanelLeft :size="16" />
            </button>
            <template v-if="viewState === 'viewing' && activePage">
              <h3 class="page-heading">{{ activePage.title }}</h3>
            </template>
            <template v-else-if="viewState === 'editing'">
              <h3 class="page-heading">{{ t('wiki.editing', { title: activePage?.title }) }}</h3>
            </template>
            <template v-else-if="viewState === 'creating'">
              <h3 class="page-heading">{{ t('wiki.newPageTitle') }}</h3>
            </template>
            <template v-else-if="viewState === 'viewing-raw'">
              <h3 class="page-heading">{{ activeRawFile }}</h3>
            </template>
            <template v-else-if="viewState === 'new-folder'">
              <h3 class="page-heading">{{ t('wiki.newFolderTitle') }}</h3>
            </template>
          </div>
          <div class="header-center">
            <div class="header-search">
              <Search :size="13" />
              <input
                v-model="wikiSearch"
                type="text"
                class="search-input"
                :placeholder="t('wiki.searchPlaceholder')"
              />
            </div>
          </div>
          <div v-if="viewState === 'viewing' && activePage" class="header-actions">
            <button class="action-btn" @click="viewState = 'editing'" :title="t('wiki.editPage')">
              <Pencil :size="14" />
              {{ t('common.edit') }}
            </button>
            <button class="action-btn danger" @click="handleDelete" :title="t('wiki.deletePage')">
              <Trash2 :size="14" />
            </button>
          </div>
          <div v-if="viewState === 'viewing-raw' && activeRawFile" class="header-actions">
            <button class="action-btn danger" @click="handleDeleteRaw" :title="t('wiki.deleteFile')">
              <Trash2 :size="14" />
            </button>
          </div>
        </div>

        <div class="content-scroll" :class="{ 'is-editing': viewState === 'editing' || viewState === 'creating' }">
          <!-- Empty state -->
          <div v-if="viewState === 'empty'" class="content-empty">
            <BookOpen :size="32" />
            <p>{{ t('wiki.selectPage') }}</p>
          </div>

          <!-- Loading -->
          <div v-else-if="isLoading" class="content-empty">
            <span class="loading">{{ t('common.loading') }}</span>
          </div>

          <!-- Viewing wiki page -->
          <div v-else-if="viewState === 'viewing' && activePage" class="content-body">
            <MarkdownContent :content="activePage.content" />
            <div class="content-footer">
              <div class="footer-tags">
                <span v-for="tag in activePage.tags" :key="tag" class="footer-tag">{{ tag }}</span>
              </div>
              <div class="footer-meta">
                <span class="meta-item">{{ activePage.source_type }}</span>
                <span class="meta-item">v{{ activePage.version }}</span>
                <span class="meta-item">{{ formatDate(activePage.updated_at) }}</span>
              </div>
            </div>
          </div>

          <!-- Viewing raw file -->
          <div v-else-if="viewState === 'viewing-raw' && activeRawFile" class="content-body">
            <img v-if="isImage(activeRawFile)" :src="rawFileUrl(project, activeRawFile)" class="raw-preview-img" />
            <iframe
              v-else-if="isPdf(activeRawFile)"
              :src="rawFileUrl(project, activeRawFile)"
              class="raw-preview-pdf"
            />
            <MarkdownContent v-else-if="isText(activeRawFile)" :content="rawTextContent" />
            <div v-else class="raw-download">
              <File :size="24" />
              <a :href="rawFileUrl(project, activeRawFile)" download class="download-link">{{ activeRawFile }}</a>
            </div>
          </div>

          <!-- Creating wiki page -->
          <div v-else-if="viewState === 'creating'" class="content-body">
            <div class="create-form">
              <div class="form-field">
                <label>Path</label>
                <input v-model="createForm.slug" type="text" placeholder="guide/getting-started" />
              </div>
              <div class="form-field">
                <label>Title</label>
                <input v-model="createForm.title" type="text" placeholder="Getting Started" />
              </div>
              <div class="form-field">
                <label>Source Type</label>
                <select v-model="createForm.source_type">
                  <option value="manual">Manual</option>
                  <option value="guide">Guide</option>
                  <option value="api_ref">API Reference</option>
                  <option value="custom">Custom</option>
                </select>
              </div>
              <div class="form-field">
                <label>Tags (comma-separated)</label>
                <input v-model="createForm.tagsInput" type="text" placeholder="tag1, tag2" />
              </div>
              <AutoDownEditor :content="createForm.content" @save="handleCreateSave" @cancel="cancelCreate" />
            </div>
          </div>

          <!-- Editing wiki page -->
          <div v-else-if="viewState === 'editing' && activePage" class="content-body">
            <AutoDownEditor :content="activePage.content" @save="handleEditSave" @cancel="viewState = 'viewing'" />
          </div>

          <!-- New folder -->
          <div v-else-if="viewState === 'new-folder'" class="content-body">
            <div class="create-form">
              <div class="form-field">
                <label>Folder Path</label>
                <input v-model="newFolderPath" type="text" placeholder="datasheets/stm32" @keydown.enter="createFolder" />
              </div>
              <div class="form-actions">
                <button class="action-btn primary" @click="createFolder">
                  <Check :size="13" />
                  {{ t('common.create') }}
                </button>
                <button class="action-btn" @click="viewState = 'empty'">{{ t('common.cancel') }}</button>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  BookOpen, PanelLeft, Plus, Pencil, Trash2, FileText, File,
  ChevronRight, ChevronDown, FolderInput, FolderPlus, Check, Search,
} from 'lucide-vue-next'
import { useWiki } from '@/composables/useWiki'
import { useProject } from '@/composables/useProject'
import { authFetch } from '@/composables/useAuth'
import MarkdownContent from '@/components/MarkdownContent.vue'
import AutoDownEditor from '@/components/editors/autodown/core/AutoDownEditor.vue'
import TreeView from '@/components/TreeView.vue'
import DropZone from '@/components/DropZone.vue'
import type { TreeNode } from '@/types/wiki'

const {
  pages, activePage, isLoading, wikiTree, rawTree, uploadProgress,
  loadPages, loadPage, createPage, updatePage, deletePage,
  loadWikiTree, loadRawTree, uploadRawFiles, deleteRawFile, createRawFolder, rawFileUrl,
} = useWiki()
const { t } = useI18n()

const { projectName } = useProject()

const WIKI_SIDEBAR_KEY = 'autoforge-wiki-sidebar-collapsed'

type ViewState = 'empty' | 'viewing' | 'editing' | 'creating' | 'viewing-raw' | 'new-folder'
const viewState = ref<ViewState>('empty')
const sidebarCollapsed = ref(localStorage.getItem(WIKI_SIDEBAR_KEY) === 'true')
const rawExpanded = ref(true)
const wikiExpanded = ref(true)
const wikiSearch = ref('')
const activeRawPath = ref('')
const activeWikiPath = ref('')
const activeRawFile = ref('')
const rawTextContent = ref('')
const newFolderPath = ref('')
const newFolderTarget = ref<'raw' | 'wiki'>('raw')

const project = computed(() => projectName.value ?? 'unknown')

const createForm = ref({
  slug: '',
  title: '',
  content: '',
  source_type: 'manual' as 'manual' | 'guide' | 'api_ref' | 'custom',
  tagsInput: '',
})

watch(sidebarCollapsed, (v) => {
  localStorage.setItem(WIKI_SIDEBAR_KEY, String(v))
})

function formatDate(ts: number): string {
  return new Date(ts * 1000).toLocaleDateString()
}

function filterTree(nodes: TreeNode[], query: string): TreeNode[] {
  const q = query.trim().toLowerCase()
  if (!q) return nodes
  const result: TreeNode[] = []
  for (const node of nodes) {
    const nameMatches = node.name.toLowerCase().includes(q)
    let filteredChildren: TreeNode[] | undefined
    if (node.children) {
      filteredChildren = filterTree(node.children, q)
    }
    if (nameMatches || (filteredChildren && filteredChildren.length > 0)) {
      result.push({ ...node, children: filteredChildren?.length ? filteredChildren : undefined })
    }
  }
  return result
}

const filteredWikiTree = computed(() => filterTree(wikiTree.value, wikiSearch.value))
const filteredRawTree = computed(() => filterTree(rawTree.value, wikiSearch.value))

function isImage(path: string): boolean {
  return /\.(png|jpe?g|gif|svg|webp|bmp|ico)$/i.test(path)
}

function isPdf(path: string): boolean {
  return /\.pdf$/i.test(path)
}

function isText(path: string): boolean {
  return /\.(md|txt|csv|json|xml|yaml|yml|html|css|js|ts|rs|toml|sh|bat|py)$/i.test(path)
}

// ─── Wiki Operations ─────────────────────────────────────────────────────

async function selectWikiNode(payload: { path: string; type: string }) {
  if (payload.type !== 'file') return
  activeWikiPath.value = payload.path
  activeRawPath.value = ''
  await loadPage(project.value, payload.path)
  viewState.value = 'viewing'
}

function startCreate() {
  createForm.value = {
    slug: '',
    title: '',
    content: '',
    source_type: 'manual',
    tagsInput: '',
  }
  viewState.value = 'creating'
}

function cancelCreate() {
  viewState.value = activePage.value ? 'viewing' : 'empty'
}

async function handleCreateSave(content: string) {
  const form = createForm.value
  const slug = form.slug.trim()
  const title = form.title.trim()
  if (!slug || !title) {
    alert('Path and title are required.')
    return
  }
  const tags = form.tagsInput.split(',').map(t => t.trim()).filter(Boolean)
  await createPage(project.value, {
    slug,
    title,
    content,
    source_type: form.source_type,
    tags,
  })
  activeWikiPath.value = slug
  viewState.value = 'viewing'
}

async function handleEditSave(content: string) {
  if (!activePage.value) return
  await updatePage(project.value, activePage.value.slug, { content })
  viewState.value = 'viewing'
}

async function handleDelete() {
  if (!activePage.value) return
  if (!confirm(`Delete "${activePage.value.title}"?`)) return
  const slug = activePage.value.slug
  await deletePage(project.value, slug)
  activeWikiPath.value = ''
  viewState.value = 'empty'
}

// ─── Raw Operations ──────────────────────────────────────────────────────

async function selectRawNode(payload: { path: string; type: string }) {
  if (payload.type !== 'file') return
  activeRawPath.value = payload.path
  activeWikiPath.value = ''
  activeRawFile.value = payload.path
  if (isText(payload.path)) {
    const resp = await authFetch(rawFileUrl(project.value, payload.path))
    rawTextContent.value = await resp.text()
  }
  viewState.value = 'viewing-raw'
}

async function handleRawDrop(files: File[]) {
  await uploadRawFiles(project.value, files)
}

async function handleDeleteRaw() {
  if (!activeRawFile.value) return
  if (!confirm(`Delete "${activeRawFile.value}"?`)) return
  await deleteRawFile(project.value, activeRawFile.value)
  activeRawFile.value = ''
  activeRawPath.value = ''
  viewState.value = 'empty'
}

function startNewFolder(target: 'raw' | 'wiki') {
  newFolderTarget.value = target
  newFolderPath.value = ''
  viewState.value = 'new-folder'
}

async function createFolder() {
  const path = newFolderPath.value.trim()
  if (!path) return
  await createRawFolder(project.value, path)
  viewState.value = 'empty'
}

// ─── Lifecycle ───────────────────────────────────────────────────────────

onMounted(() => {
  if (project.value && project.value !== 'unknown') {
    loadPages(project.value)
    loadWikiTree(project.value)
    loadRawTree(project.value)
  }
})

watch(project, (val) => {
  if (val && val !== 'unknown') {
    loadPages(val)
    loadWikiTree(val)
    loadRawTree(val)
    viewState.value = 'empty'
  }
})
</script>

<style scoped>
.wiki-view {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.wiki-body {
  display: flex;
  flex: 1;
  overflow: hidden;
}

/* ─── Sidebar ─────────────────────────────────────────── */

.wiki-nav {
  width: 240px;
  min-width: 240px;
  border-right: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.02);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: width 0.2s ease, min-width 0.2s ease;
}

.wiki-nav.collapsed {
  width: 0;
  min-width: 0;
  border-right: none;
}

.wiki-nav-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  height: 48px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--af-border);
}

.wiki-nav-title {
  font-size: 0.95rem;
  font-weight: 500;
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  line-height: 1;
  flex: 1;
}

.wiki-nav-actions {
  display: flex;
  gap: 0.25rem;
}

.nav-icon-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  color: var(--af-muted);
  cursor: pointer;
  padding: 0.2rem;
  border-radius: 4px;
}

.nav-icon-btn:hover {
  color: var(--af-fg);
  background: hsl(var(--muted-foreground) / 0.08);
}

.wiki-nav-list {
  flex: 1;
  overflow-y: auto;
}

/* ─── Tree Section ────────────────────────────────────── */

.tree-section {
  border-bottom: 1px solid var(--af-border);
}

.tree-section-header {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.5rem 0.75rem;
  cursor: pointer;
  color: var(--af-muted);
  font-size: 0rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  user-select: none;
}

.tree-section-header:hover {
  color: var(--af-fg);
  background: hsl(var(--muted-foreground) / 0.03);
}

.tree-section-title {
  flex: 1;
}

.section-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  background: none;
  border: none;
  color: var(--af-muted);
  cursor: pointer;
  padding: 0.15rem;
  border-radius: 3px;
  opacity: 0;
  transition: opacity 0.15s;
}

.tree-section-header:hover .section-btn {
  opacity: 1;
}

.section-btn:hover {
  color: var(--af-fg);
  background: hsl(var(--muted-foreground) / 0.08);
}

.tree-section-body {
  padding: 0.25rem 0;
}

.tree-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.25rem;
  padding: 0.75rem;
  color: var(--af-muted);
  font-size: 0.83rem;
}

/* ─── Content ─────────────────────────────────────────── */

.wiki-content {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.content-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  height: 48px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--af-border);
  gap: 1rem;
  position: relative;
}

.header-left {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.page-heading {
  font-size: 0.83rem;
  font-weight: 500;
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  line-height: 1;
  margin: 0;
}

.header-actions {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.header-center {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  justify-content: center;
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
}

.header-search {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  width: 100%;
  max-width: 320px;
  padding: 0.35rem 0.75rem;
  background: hsl(var(--muted-foreground) / 0.06);
  border: 1px solid hsl(var(--muted-foreground) / 0.12);
  border-radius: 6px;
  color: var(--af-muted);
  transition: border-color 0.15s, background 0.15s;
}

.header-search:focus-within {
  border-color: hsl(var(--primary) / 0.35);
  background: hsl(var(--muted-foreground) / 0.04);
}

.header-search svg {
  color: var(--af-muted);
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  background: transparent;
  border: none;
  outline: none;
  color: var(--af-fg);
  font-size: 0.88rem;
  font-family: inherit;
  min-width: 0;
  width: 100%;
}

.search-input::placeholder {
  color: var(--af-muted);
  font-size: 0.88rem;
}

.action-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.4rem 0.7rem;
  font-size: 0.83rem;
  border-radius: 6px;
  border: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.04);
  color: var(--af-fg);
  cursor: pointer;
  transition: all 0.15s;
}

.action-btn:hover {
  background: hsl(var(--muted-foreground) / 0.08);
  border-color: hsl(var(--primary) / 0.3);
}

.action-btn.primary {
  background: hsl(var(--primary));
  color: white;
  border-color: transparent;
}

.action-btn.danger:hover {
  color: hsl(var(--af-error));
  border-color: hsl(var(--af-error) / 0.3);
  background: hsl(var(--af-error) / 0.06);
}

.content-scroll {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  padding: 1rem;
}

.content-scroll.is-editing {
  overflow: hidden;
}

.content-scroll.is-editing .content-body {
  height: 100%;
  overflow: hidden;
}

.content-scroll.is-editing .autodown-editor {
  min-height: 0;
}

.content-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--af-muted);
  gap: 0.5rem;
}

.loading {
  font-size: 0.98rem;
  color: var(--af-muted);
}

.content-body {
  flex: 1;
  width: 100%;
  max-width: 960px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
}

.content-footer {
  margin-top: 1.5rem;
  padding-top: 0.75rem;
  border-top: 1px solid var(--af-border);
  display: flex;
  align-items: center;
  justify-content: space-between;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.footer-tags {
  display: flex;
  gap: 0.3rem;
  flex-wrap: wrap;
}

.footer-tag {
  font-size: 0.78rem;
  padding: 0.15rem 0.5rem;
  border-radius: 4px;
  background: hsl(var(--primary) / 0.08);
  color: hsl(var(--primary));
}

.footer-meta {
  display: flex;
  gap: 0.6rem;
}

.meta-item {
  font-size: 0.78rem;
  color: var(--af-muted);
}

/* ─── Raw Preview ─────────────────────────────────────── */

.raw-preview-img {
  max-width: 100%;
  border-radius: 6px;
}

.raw-preview-pdf {
  width: 100%;
  height: 70vh;
  border: 1px solid var(--af-border);
  border-radius: 6px;
}

.raw-download {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.5rem;
  padding: 2rem;
  color: var(--af-muted);
}

.download-link {
  color: hsl(var(--primary));
  font-size: 0.93rem;
  text-decoration: none;
}

.download-link:hover {
  text-decoration: underline;
}

/* ─── Create Form ─────────────────────────────────────── */

.create-form {
  max-width: 960px;
  margin: 0 auto;
}

.form-field {
  margin-bottom: 0.75rem;
}

.form-field label {
  display: block;
  font-size: 0.83rem;
  font-weight: 600;
  color: var(--af-muted);
  margin-bottom: 0.25rem;
}

.form-field input,
.form-field select {
  width: 100%;
  padding: 0.45rem 0.6rem;
  font-size: 0rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: var(--af-card);
  color: var(--af-fg);
  outline: none;
}

.form-field input:focus,
.form-field select:focus {
  border-color: hsl(var(--primary) / 0.5);
}

.form-actions {
  display: flex;
  gap: 0.5rem;
}

/* ─── Mobile ──────────────────────────────────────────── */

@media (max-width: 768px) {
  .wiki-nav {
    position: fixed;
    left: 0;
    top: 0;
    bottom: 0;
    z-index: 50;
    background: var(--af-bg);
    box-shadow: 2px 0 8px rgba(0, 0, 0, 0.1);
  }

  .wiki-nav.collapsed {
    width: 0;
    min-width: 0;
    overflow: hidden;
  }
}
</style>
