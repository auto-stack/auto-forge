<template>
  <div class="explorer-view">
    <div class="explorer-body">
      <!-- Sidebar -->
      <div class="explorer-nav" :class="{ collapsed: sidebarCollapsed }">
        <div class="explorer-nav-header">
          <span class="explorer-nav-title">{{ t('explorer.title') }}</span>
          <div class="explorer-nav-actions">
            <button class="nav-icon-btn" @click="refreshTree" :title="t('explorer.refresh')">
              <RefreshCw :size="14" />
            </button>
            <button class="nav-icon-btn" @click="sidebarCollapsed = !sidebarCollapsed" :title="t('explorer.toggleSidebar')">
              <PanelLeft :size="14" />
            </button>
            <button class="nav-icon-btn" @click="closeProject" :title="t('explorer.closeProject')">
              <X :size="14" />
            </button>
          </div>
        </div>
        <div class="explorer-nav-list">
          <div v-if="treeLoading" class="tree-empty">
            <span class="loading">{{ t('explorer.loading') }}</span>
          </div>
          <div v-else-if="tree.length === 0" class="tree-empty">
            <FolderOpen :size="14" />
            <span>{{ t('explorer.noFiles') }}</span>
          </div>
          <TreeView
            v-for="node in filteredTree"
            v-else
            :key="node.path"
            :node="node"
            :active-path="activePath"
            @select="onSelectNode"
          />
        </div>
      </div>

      <!-- Content pane -->
      <div class="explorer-content">
        <div class="content-header">
          <div class="header-left">
            <button v-if="sidebarCollapsed" class="nav-icon-btn" @click="sidebarCollapsed = false" :title="t('explorer.showSidebar')">
              <PanelLeft :size="16" />
            </button>
            <h3 v-if="activeFile" class="page-heading">{{ activeFileName }}</h3>
          </div>
          <div class="header-center">
            <div class="header-search">
              <Search :size="13" />
              <input
                v-model="explorerSearch"
                type="text"
                class="search-input"
                :placeholder="t('explorer.searchPlaceholder')"
              />
            </div>
          </div>
        </div>

        <div class="content-scroll">
          <!-- Empty state -->
          <div v-if="!activeFile" class="content-empty">
            <Files :size="32" />
            <p>{{ t('explorer.selectFile') }}</p>
          </div>

          <!-- Loading -->
          <div v-else-if="fileLoading" class="content-empty">
            <span class="loading">{{ t('explorer.loading') }}</span>
          </div>

          <!-- Image -->
          <div v-else-if="isImage(activeFile)" class="content-body">
            <img :src="fileDataUrl" class="file-preview-img" />
          </div>

          <!-- PDF -->
          <div v-else-if="isPdf(activeFile)" class="content-body">
            <iframe :src="fileDataUrl" class="file-preview-pdf" />
          </div>

          <!-- Text / Code -->
          <div v-else-if="isText(activeFile)" class="content-body">
            <pre class="code-block"><code>{{ fileContent }}</code></pre>
          </div>

          <!-- Other -->
          <div v-else class="content-empty">
            <File :size="24" />
            <p>{{ t('explorer.previewNotAvailable') }}</p>
            <a v-if="fileDataUrl" :href="fileDataUrl" download class="download-link">{{ t('explorer.download', { name: activeFileName }) }}</a>
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
  PanelLeft, RefreshCw, FolderOpen, Files, File, X, Search,
} from 'lucide-vue-next'
import { useProject } from '@/composables/useProject'
import TreeView from '@/components/TreeView.vue'
import type { TreeNode } from '@/types/wiki'
import { authFetch } from '../composables/useAuth'

const { t } = useI18n()
const { projectName, projectPath, closeProject } = useProject()

const EXPLORER_SIDEBAR_KEY = 'autoforge-explorer-sidebar-collapsed'

const sidebarCollapsed = ref(localStorage.getItem(EXPLORER_SIDEBAR_KEY) === 'true')
const tree = ref<TreeNode[]>([])
const treeLoading = ref(false)
const explorerSearch = ref('')
const activePath = ref('')
const activeFile = ref('')
const activeFileName = ref('')
const fileContent = ref('')
const fileDataUrl = ref('')
const fileLoading = ref(false)

watch(sidebarCollapsed, (v) => {
  localStorage.setItem(EXPLORER_SIDEBAR_KEY, String(v))
})

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

const filteredTree = computed(() => filterTree(tree.value, explorerSearch.value))

function isImage(path: string): boolean {
  return /\.(png|jpe?g|gif|svg|webp|bmp|ico)$/i.test(path)
}

function isPdf(path: string): boolean {
  return /\.pdf$/i.test(path)
}

function isText(path: string): boolean {
  return /\.(md|txt|csv|json|xml|yaml|yml|html|css|js|ts|rs|toml|sh|bat|py|vue|tsx?|jsx?|ad|lock)$/i.test(path)
}

async function loadTree() {
  treeLoading.value = true
  try {
    const resp = await authFetch('/api/forge/project/tree')
    if (!resp.ok) throw new Error(`Failed: ${resp.status}`)
    tree.value = await resp.json()
  } catch (e) {
    tree.value = []
  } finally {
    treeLoading.value = false
  }
}

function refreshTree() {
  loadTree()
}

async function onSelectNode(payload: { path: string; type: string }) {
  if (payload.type !== 'file') return
  activePath.value = payload.path
  activeFile.value = payload.path
  activeFileName.value = payload.path.split(/[\\/]/).pop() ?? payload.path
  fileContent.value = ''
  fileDataUrl.value = ''
  fileLoading.value = true

  try {
    const resp = await authFetch(`/api/forge/project/file?path=${encodeURIComponent(payload.path)}`)
    if (!resp.ok) throw new Error(`Failed: ${resp.status}`)

    if (isImage(payload.path) || isPdf(payload.path)) {
      const blob = await resp.blob()
      fileDataUrl.value = URL.createObjectURL(blob)
    } else if (isText(payload.path)) {
      fileContent.value = await resp.text()
    } else {
      const blob = await resp.blob()
      fileDataUrl.value = URL.createObjectURL(blob)
    }
  } catch (e) {
    fileContent.value = `Error loading file: ${e instanceof Error ? e.message : String(e)}`
  } finally {
    fileLoading.value = false
  }
}

onMounted(() => {
  if (projectPath.value) {
    loadTree()
  }
})

watch(projectPath, (val) => {
  if (val) {
    loadTree()
    activeFile.value = ''
    activePath.value = ''
    fileContent.value = ''
    fileDataUrl.value = ''
  }
})
</script>

<style scoped>
.explorer-view {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.explorer-body {
  display: flex;
  flex: 1;
  overflow: hidden;
}

/* ─── Sidebar ─────────────────────────────────────────── */

.explorer-nav {
  width: 260px;
  min-width: 260px;
  border-right: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.02);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  transition: width 0.2s ease, min-width 0.2s ease;
}

.explorer-nav.collapsed {
  width: 0;
  min-width: 0;
  border-right: none;
}

.explorer-nav-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  height: 48px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--af-border);
}

.explorer-nav-title {
  font-size: 0.95rem;
  font-weight: 500;
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  line-height: 1;
  flex: 1;
}

.explorer-nav-actions {
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

.explorer-nav-list {
  flex: 1;
  overflow-y: auto;
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

.explorer-content {
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

.header-center {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex: 1;
  justify-content: center;
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

.content-scroll {
  flex: 1;
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  padding: 1rem;
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

.content-body {
  flex: 1;
  width: 100%;
  max-width: 960px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
}

.loading {
  font-size: 0.98rem;
  color: var(--af-muted);
}

.file-preview-img {
  max-width: 100%;
  border-radius: 6px;
}

.file-preview-pdf {
  width: 100%;
  height: 70vh;
  border: 1px solid var(--af-border);
  border-radius: 6px;
}

.code-block {
  background: hsl(var(--muted-foreground) / 0.04);
  border: 1px solid var(--af-border);
  border-radius: 6px;
  padding: 1rem;
  overflow-x: auto;
  font-family: 'SF Mono', Monaco, 'Cascadia Code', 'Roboto Mono', Consolas, monospace;
  font-size: 0.85rem;
  line-height: 1.5;
  color: var(--af-fg);
  white-space: pre;
  margin: 0;
}

.download-link {
  color: hsl(var(--primary));
  font-size: 0.93rem;
  text-decoration: none;
}

.download-link:hover {
  text-decoration: underline;
}

/* ─── Mobile ──────────────────────────────────────────── */

@media (max-width: 768px) {
  .explorer-nav {
    position: fixed;
    left: 0;
    top: 0;
    bottom: 0;
    z-index: 50;
    background: var(--af-bg);
    box-shadow: 2px 0 8px rgba(0, 0, 0, 0.1);
  }

  .explorer-nav.collapsed {
    width: 0;
    min-width: 0;
    overflow: hidden;
  }
}
</style>
