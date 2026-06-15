<template>
  <div class="specs-view">
    <div class="specs-body">
      <!-- Sidebar -->
      <div class="section-nav" :class="{ collapsed: sectionNavCollapsed }">
        <div class="section-nav-header">
          <span class="section-nav-title">{{ t('specs.title') }}</span>
          <button
            class="section-nav-collapse-btn"
            @click="sectionNavCollapsed = !sectionNavCollapsed"
            :title="t('specs.toggleSidebar')"
          >
            <PanelLeft :size="14" />
          </button>
        </div>
        <!-- Top-level Overview -->
        <div
          class="overview-entry"
          :class="{ active: showOverview }"
          @click="selectOverview()"
        >
          <BookOpen :size="15" />
          <span>{{ t('specs.overview') }}</span>
        </div>

        <!-- Module Accordion -->
        <div class="module-accordion">
          <details
            v-for="mod in filteredModuleTree"
            :key="mod.name"
            class="filter-drawer module-drawer"
            :open="expandedModules.has(mod.name)"
          >
            <summary class="filter-drawer-title module-summary" @click.prevent="toggleModuleExpand(mod.name)">
              <span class="module-name">{{ mod.name }}</span>
              <span class="module-count">{{ mod.total }}</span>
            </summary>
            <div class="filter-drawer-body module-types">
              <!-- Module Outline -->
              <div
                class="section-nav-item type-nav-item outline-nav-item"
                :class="{ active: activeModuleOutline === mod.name }"
                @click="selectModuleOutline(mod.name)"
              >
                <div class="section-top">
                  <FileText :size="14" />
                  <span class="section-name">Outline</span>
                </div>
              </div>
              <div
                v-for="type in mod.types"
                :key="type.id"
                class="section-nav-item type-nav-item"
                :class="{ active: activeModule === mod.name && activeSection === type.id && !activeModuleOutline }"
                @click="selectModuleSection(mod.name, type.id)"
              >
                <div class="section-top">
                  <span class="section-name">{{ type.title }}</span>
                </div>
                <div class="section-meta">
                  <span class="section-count">{{ type.count }} items</span>
                </div>
              </div>
            </div>
          </details>
        </div>

        <!-- Stack Drawer -->
        <details class="filter-drawer" open v-if="allStacks.length > 0">
          <summary class="filter-drawer-title">Stack</summary>
          <div class="filter-drawer-body pills">
            <button
              class="filter-pill"
              :class="{ active: selectedStacks.length === 0 }"
              @click="clearStacks"
            >All</button>
            <button
              v-for="stack in allStacks"
              :key="stack"
              class="filter-pill"
              :class="{ active: selectedStacks.includes(stack) }"
              @click="toggleStack(stack)"
            >{{ stack }}</button>
          </div>
        </details>
      </div>

      <!-- Content pane -->
      <div class="section-editor">
        <div class="content-header">
          <div class="header-left">
            <button v-if="sectionNavCollapsed" class="section-nav-toggle-btn" @click="sectionNavCollapsed = false" :title="t('specs.showSections')">
              <PanelLeft :size="16" />
            </button>
            <div class="header-breadcrumb">
              <span v-if="projectName" class="breadcrumb-project">{{ projectName }}</span>
              <span v-if="projectName && (showOverview || activeModule || activeModuleOutline)" class="breadcrumb-sep">/</span>
              <template v-if="showOverview">
                <span class="breadcrumb-section">Overview</span>
              </template>
              <template v-else-if="activeModuleOutline">
                <span class="breadcrumb-module">{{ activeModuleOutline }}</span>
                <span class="breadcrumb-sep">/</span>
                <span class="breadcrumb-section">Outline</span>
              </template>
              <template v-else>
                <span v-if="activeModule" class="breadcrumb-module">{{ activeModule }}</span>
                <span v-if="activeModule && currentSection" class="breadcrumb-sep">/</span>
                <span v-if="currentSection" class="breadcrumb-section">{{ currentSection.title }}</span>
              </template>
            </div>
          </div>

          <div class="header-search">
            <Search :size="14" />
            <input
              v-model="specSearch"
              type="text"
              class="search-input"
              :placeholder="t('specs.searchPlaceholder')"
            />
          </div>

          <div class="specs-actions">
            <button class="specs-btn" @click="triggerDriftCheck">
              <RefreshCw :size="14" />
              {{ t('specs.driftCheck') }}
            </button>
            <button class="specs-btn" @click="rebuildRelations">
              <Link2 :size="14" />
              {{ t('specs.rebuildLinks') }}
            </button>
          </div>
        </div>

        <div class="editor-scroll">
          <div v-if="showOverview && overviewExists" class="editor-pane">
            <div class="editor-header">
              <h3>Overview</h3>
            </div>
            <MarkdownContent :content="overviewContent" />
          </div>

          <div v-else-if="activeModuleOutline && moduleOutlineExists" class="editor-pane">
            <div class="editor-header">
              <h3>{{ activeModuleOutline }} — Outline</h3>
            </div>
            <MarkdownContent :content="moduleOutlineContent" />
          </div>

          <div v-else-if="currentSection" class="editor-pane">
            <!-- Active gate banner for this section -->
            <GateBanner
              v-if="sectionGate"
              :gate="sectionGate"
              @approve="onGateApprove"
              @reject="onGateReject"
              @open-in-chat="onGateOpenInChat"
            />

            <div class="editor-header">
              <h3>{{ currentSection.title }}</h3>
              <div class="editor-header-right">
                <div class="editor-badges">
                  <StatusBadge :status="currentSection.status" />
                </div>
              </div>
            </div>

            <!-- Toolbar: add item -->
            <div class="section-toolbar">
              <button class="add-btn" @click="addItem">
                <Plus :size="14" />
                {{ t('specs.add', { type: sectionTypeLabel }) }}
              </button>
            </div>

            <!-- Category-specific renderer -->
            <template v-if="currentSection?.section_type === 'goals'">
              <GoalsTable
                :items="filteredItems"
                :project="project"
                @open-detail="openDetailModal"
              />
              <GoalDetailModal
                :item="detailItem"
                :project="project"
                section-type="goals"
                :is-editing="detailEditing"
                @close="closeDetailModal"
                @edit="startDetailEdit"
                @save="handleDetailSave"
                @cancel-edit="cancelDetailEdit"
                @status-change="handleStatusChange"
                @delete="handleDelete"
                @jump="jumpToItem"
              />
            </template>
            <component
              v-else
              :is="categoryComponent"
              :items="filteredItems"
              :project="project"
              :expanded-id="activeItemId"
              :editing-id="editingItemId"
              @toggle="toggleItem"
              @jump="jumpToItem"
              @edit="startEditItem"
              @status-change="handleStatusChange"
              @delete="handleDelete"
              @save="handleSave"
              @cancel-edit="cancelEdit"
            />
          </div>

          <div v-else-if="isLoading" class="editor-empty">
            <span class="loading">{{ t('specs.loading') }}</span>
          </div>
          <div v-else class="editor-empty">
            <BookOpen :size="32" />
            <p>{{ t('specs.selectSection') }}</p>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, watch } from 'vue'
import {
  RefreshCw, Search, PanelLeft, BookOpen, Plus, Link2, FileText
} from 'lucide-vue-next'
import { useI18n } from 'vue-i18n'
import { useSpecs } from '@/composables/useSpecs'
import { useGateInbox } from '@/composables/useGateInbox'
import { useProject } from '@/composables/useProject'
import { useViewState } from '@/composables/useViewState'
import type { SpecsSection, SpecItem, SectionType, Status } from '@/types/specs'
import StatusBadge from '@/components/StatusBadge.vue'
import GateBanner from '@/components/GateBanner.vue'
import GoalDetailModal from '@/components/GoalDetailModal.vue'
import MarkdownContent from '@/components/MarkdownContent.vue'
import { ITEM_TEMPLATES, getDefaultStatus, getNextId } from '@/utils/itemTemplates'

// Category components
import GoalsTable from '@/components/category/GoalsTable.vue'
import ArchitectureCards from '@/components/category/ArchitectureCards.vue'
import DesignCards from '@/components/category/DesignCards.vue'
import PlanCards from '@/components/category/PlanCards.vue'
import TestsCards from '@/components/category/TestsCards.vue'
import ReviewCards from '@/components/category/ReviewCards.vue'
import ReportCards from '@/components/category/ReportCards.vue'
import { authFetch } from '../composables/useAuth'

const { t } = useI18n()

const { document, isLoading, error, loadDocument, loadOverview, loadModuleOutline, saveDocument, findItemById, findSectionByItemId, rebuildRelations: apiRebuildRelations } = useSpecs()
const { gates: pendingGates, resolveGate: resolveGateInbox } = useGateInbox()
const { projectName: activeProjectName } = useProject()
const viewState = useViewState()

const SPECS_SIDEBAR_KEY = 'autoforge-specs-sidebar-collapsed'

const activeSection = ref<string>('goals')
const activeModule = ref<string>('')
const showOverview = ref(true)
const overviewContent = ref('')
const overviewExists = ref(false)
const activeModuleOutline = ref<string | null>(null)
const moduleOutlineContent = ref('')
const moduleOutlineExists = ref(false)
const activeItemId = ref<string | null>(null)
const editingItemId = ref<string | null>(null)
const detailItem = ref<SpecItem | null>(null)
const detailEditing = ref(false)
const project = computed(() => activeProjectName.value ?? 'unknown')
const specSearch = ref('')
const sectionNavCollapsed = ref(localStorage.getItem(SPECS_SIDEBAR_KEY) === 'true')
const flashItemId = ref<string | null>(null)
const selectedStacks = ref<string[]>([])
const expandedModules = ref<Set<string>>(new Set())

watch(sectionNavCollapsed, (v) => {
  localStorage.setItem(SPECS_SIDEBAR_KEY, String(v))
})

const projectName = computed(() => {
  const p = project.value
  if (!p || p === '.') return null
  const parts = p.replace(/\\/g, '/').split('/').filter(Boolean)
  return parts.length > 0 ? parts[parts.length - 1] : null
})

const DEFAULT_SECTIONS: SpecsSection[] = [
  { id: 'goals', section_type: 'goals', title: '🎯 Goals', items: [], content: '', status: 'empty', last_modified: Date.now() },
  { id: 'architecture', section_type: 'architecture', title: '🏗️ Architecture', items: [], content: '', status: 'empty', last_modified: Date.now() },
  { id: 'designs', section_type: 'designs', title: '🎨 Designs', items: [], content: '', status: 'empty', last_modified: Date.now() },
  { id: 'plans', section_type: 'plans', title: '📅 Plans', items: [], content: '', status: 'empty', last_modified: Date.now() },
  { id: 'tests', section_type: 'tests', title: '🧪 Tests', items: [], content: '', status: 'empty', last_modified: Date.now() },
  { id: 'reviews', section_type: 'reviews', title: '📝 Reviews', items: [], content: '', status: 'empty', last_modified: Date.now() },
  { id: 'reports', section_type: 'reports', title: '📊 Reports', items: [], content: '', status: 'empty', last_modified: Date.now() },
]

const sections = computed(() => {
  const loaded = document.value?.sections
  if (loaded && loaded.length > 0) return loaded
  return DEFAULT_SECTIONS
})

// Extract module name from item id, e.g. "AgentConfig-G1" -> "agent-config"
function idToModule(id: string): string | null {
  const prefix = id.split('-')[0]
  if (!prefix) return null
  let result = ''
  for (let i = 0; i < prefix.length; i++) {
    const c = prefix[i]
    const isUpper = c >= 'A' && c <= 'Z'
    if (isUpper && i > 0) {
      const prev = prefix[i - 1]
      const next = prefix[i + 1]
      const prevLower = prev >= 'a' && prev <= 'z'
      const nextLower = next >= 'a' && next <= 'z'
      if (prevLower || nextLower) {
        result += '-'
      }
    }
    result += c.toLowerCase()
  }
  return result
}

function getModule(item: SpecItem): string | null {
  // Prefer the canonical `module:<name>` tag over free-text metadata.
  if (item.tags) {
    const modTag = item.tags.find(t => t.toLowerCase().startsWith('module:'))
    if (modTag) return modTag.split(':')[1]?.trim().toLowerCase() || null
  }
  // Fall back to the explicit Module field only when it looks like a single
  // module name and not a file path or comma-separated file list.
  if (item.module && !/[\/,.]/.test(item.module)) {
    return item.module.toLowerCase()
  }
  return idToModule(item.id)
}

const moduleTree = computed(() => {
  const tree = new Map<string, Map<string, number>>()
  for (const section of sections.value) {
    for (const item of section.items) {
      const mod = getModule(item)
      if (!mod) continue
      if (!tree.has(mod)) tree.set(mod, new Map())
      const typeMap = tree.get(mod)!
      typeMap.set(section.id, (typeMap.get(section.id) || 0) + 1)
    }
  }
  const typeOrder = ['goals', 'architecture', 'designs', 'plans', 'tests', 'reviews', 'reports']
  const result = []
  for (const [modName, typeMap] of tree) {
    const types = []
    for (const typeId of typeOrder) {
      const count = typeMap.get(typeId)
      if (count !== undefined) {
        const section = sections.value.find(s => s.id === typeId)
        types.push({ id: typeId, title: section?.title || typeId, count })
      }
    }
    const total = Array.from(typeMap.values()).reduce((a, b) => a + b, 0)
    result.push({ name: modName, types, total })
  }
  return result.sort((a, b) => a.name.localeCompare(b.name))
})

const filteredModuleTree = computed(() => {
  const q = specSearch.value.trim().toLowerCase()
  if (!q) return moduleTree.value
  return moduleTree.value.filter(mod =>
    mod.name.toLowerCase().includes(q) ||
    mod.types.some(t => t.title.toLowerCase().includes(q) || t.id.toLowerCase().includes(q))
  )
})

const currentSection = computed(() =>
  document.value?.sections.find((s) => s.id === activeSection.value) ?? null
)

const sectionGate = computed(() => {
  if (!currentSection.value) return null
  return pendingGates.value.find(
    (g: { sectionId?: string; status: string }) => g.sectionId === currentSection.value!.id && g.status === 'pending'
  ) ?? null
})

function onGateApprove(gateId: string) {
  resolveGateInbox(gateId, 'approved')
}

function onGateReject(gateId: string) {
  resolveGateInbox(gateId, 'rejected')
}

function onGateOpenInChat(gateId: string) {
  alert(`Open gate ${gateId} in chat view`)
}

const categoryComponent = computed(() => {
  const type = currentSection.value?.section_type
  switch (type) {
    case 'goals': return GoalsTable
    case 'architecture': return ArchitectureCards
    case 'designs': return DesignCards
    case 'plans': return PlanCards
    case 'tests': return TestsCards
    case 'reviews': return ReviewCards
    case 'reports': return ReportCards
    default: return null
  }
})

const sectionTypeLabel = computed(() => {
  const type = currentSection.value?.section_type
  if (!type) return 'Item'
  return type.charAt(0).toUpperCase() + type.slice(1).replace('_', ' ')
})

// ─── Sidebar filter state ──────────────────────────────────────

const allStacks = computed(() => {
  const set = new Set<string>()
  for (const section of sections.value) {
    for (const item of section.items) {
      item.tags?.forEach(tag => {
        if (tag.startsWith('stack:')) set.add(tag.replace('stack:', ''))
      })
    }
  }
  return Array.from(set).sort()
})

const filteredItems = computed(() => {
  const section = currentSection.value
  if (!section) return []
  let items = section.items

  // Module filter
  if (activeModule.value) {
    items = items.filter(item => getModule(item) === activeModule.value)
  }

  // Stack filter
  if (selectedStacks.value.length > 0) {
    items = items.filter(item =>
      selectedStacks.value.some(s => item.tags?.includes(`stack:${s}`))
    )
  }

  return items
})

function toggleStack(stack: string) {
  const idx = selectedStacks.value.indexOf(stack)
  if (idx >= 0) {
    selectedStacks.value.splice(idx, 1)
  } else {
    selectedStacks.value.push(stack)
  }
}

function clearStacks() { selectedStacks.value = [] }

function toggleModuleExpand(modName: string) {
  const newSet = new Set(expandedModules.value)
  if (newSet.has(modName)) {
    newSet.delete(modName)
  } else {
    newSet.add(modName)
  }
  expandedModules.value = newSet
}

function selectModuleSection(modName: string, sectionId: string) {
  showOverview.value = false
  activeModuleOutline.value = null
  activeModule.value = modName
  activeSection.value = sectionId
}

function selectModuleOutline(modName: string) {
  showOverview.value = false
  activeModuleOutline.value = modName
  activeModule.value = modName
  activeSection.value = ''
  loadModuleOutlineContent(modName)
}

function selectOverview() {
  showOverview.value = true
  activeModuleOutline.value = null
  activeModule.value = ''
  activeSection.value = ''
  activeItemId.value = null
  viewState.setDetailPath('')
}

watch([activeModule, activeSection], () => {
  selectedStacks.value = []
})

watch(moduleTree, (tree) => {
  if (tree.length === 0) return

  // If URL has a specs detail path, restore it instead of using defaults
  if (restoreSpecsFromUrl()) return

  if (!activeModule.value && !showOverview.value && !activeModuleOutline.value) {
    activeModule.value = tree[0].name
    activeSection.value = tree[0].types[0]?.id || 'goals'
    expandedModules.value = new Set([tree[0].name])
  }
}, { immediate: true })

// Restore / sync URL detail path for specs, e.g. /forge/specs/{module}/{section}/{item}
function restoreSpecsFromUrl() {
  const detailPath = viewState.currentDetailPath.value
  if (!detailPath) return false

  const parts = detailPath.split('/')
  const modName = parts[0]
  const sectionId = parts[1] || ''
  const itemId = parts[2] || null

  const module = moduleTree.value.find((m) => m.name === modName)
  if (!module) return false

  showOverview.value = false
  activeModuleOutline.value = null
  activeModule.value = modName
  expandedModules.value = new Set([modName, ...expandedModules.value])

  if (sectionId) {
    const sectionExists = module.types.some((t) => t.id === sectionId)
    activeSection.value = sectionExists ? sectionId : (module.types[0]?.id || 'goals')
  } else {
    activeSection.value = module.types[0]?.id || 'goals'
  }

  if (itemId) {
    activeItemId.value = itemId
  }

  return true
}

watch([activeModule, activeSection, activeItemId], () => {
  if (showOverview.value) {
    viewState.setDetailPath('')
    return
  }
  if (!activeModule.value) {
    viewState.setDetailPath('')
    return
  }
  const parts = [activeModule.value]
  if (activeSection.value) {
    parts.push(activeSection.value)
  }
  if (activeItemId.value) {
    parts.push(activeItemId.value)
  }
  viewState.setDetailPath(parts.join('/'))
})

async function fetchOverview() {
  const result = await loadOverview(project.value)
  overviewContent.value = result.content
  overviewExists.value = result.exists
}

async function loadModuleOutlineContent(modName: string) {
  const result = await loadModuleOutline(project.value, modName)
  moduleOutlineContent.value = result.content
  moduleOutlineExists.value = result.exists
}

// ─────────────────────────────────────────────────────────

function handleStatusChange(payload: { id: string; status: Status }) {
  const section = currentSection.value
  if (!section) return
  const item = section.items.find((i) => i.id === payload.id)
  if (item) {
    item.status = payload.status
    item.modified_at = Date.now()
    saveSection()
  }
}

function handleSave(updated: SpecItem) {
  const section = currentSection.value
  if (!section) return

  // Validation
  const title = updated.title.trim()
  if (!title) {
    alert('Title cannot be empty.')
    return
  }

  const idx = section.items.findIndex((i) => i.id === updated.id)
  if (idx >= 0) {
    section.items[idx] = { ...updated, title }
    saveSection()
  }
}

function handleDelete(itemId: string) {
  const section = currentSection.value
  if (!section) return
  const idx = section.items.findIndex((i) => i.id === itemId)
  if (idx >= 0) {
    section.items.splice(idx, 1)
    if (activeItemId.value === itemId) activeItemId.value = null
    saveSection()
  }
}

function toggleItem(id: string) {
  activeItemId.value = activeItemId.value === id ? null : id
}

function jumpToItem(id: string) {
  const result = findItemById(id)
  if (!result) return
  const section = findSectionByItemId(id)
  if (section) {
    activeSection.value = section.id
    activeItemId.value = id
    flashItemId.value = id
    setTimeout(() => { flashItemId.value = null }, 2000)
  }
}

function startEditItem(item: SpecItem) {
  activeItemId.value = item.id
  editingItemId.value = item.id
}

function cancelEdit() {
  editingItemId.value = null
}

// ─── Goal Detail Modal ───────────────────────────────────────

function openDetailModal(item: SpecItem) {
  detailItem.value = item
  detailEditing.value = false
}

function closeDetailModal() {
  detailItem.value = null
  detailEditing.value = false
}

function startDetailEdit() {
  detailEditing.value = true
}

function cancelDetailEdit() {
  detailEditing.value = false
}

function handleDetailSave(payload: { title: string; content: string; priority: string; depends_on: string[] }) {
  if (!detailItem.value) return
  const item = detailItem.value
  const section = currentSection.value
  if (!section) return
  const idx = section.items.findIndex((i) => i.id === item.id)
  if (idx >= 0) {
    section.items[idx] = {
      ...section.items[idx],
      title: payload.title,
      content: payload.content,
      priority: payload.priority,
      depends_on: payload.depends_on,
      modified_at: Date.now(),
    }
    saveSection()
  }
  detailEditing.value = false
}

function addItem() {
  if (!currentSection.value) return
  const section = currentSection.value
  const existingIds = section.items.map((i) => i.id)
  const newId = getNextId(section.section_type, existingIds)
  const template = ITEM_TEMPLATES[section.section_type] || ''
  const newItem: SpecItem = {
    id: newId,
    title: `New ${sectionTypeLabel.value}`,
    content: template,
    status: getDefaultStatus(section.section_type),
    created_at: Date.now(),
    modified_at: Date.now(),
  }
  section.items.push(newItem)
  activeItemId.value = newItem.id
  editingItemId.value = newItem.id
  saveSection()
}

async function saveSection() {
  const section = currentSection.value
  if (!section) return
  section.content = serializeItemsToMarkdown(section)
  section.last_modified = Date.now()
  const doc = document.value
  if (doc) {
    await saveDocument(project.value, doc)
  }
  if (error.value) {
    alert('Save failed: ' + error.value)
  }
}

function serializeItemsToMarkdown(section: SpecsSection): string {
  const lines: string[] = [`## ${section.title.replace(/^[^\w]+\s*/, '')}`]
  for (const item of section.items) {
    lines.push(`### ${item.id} ${item.title}`)
    lines.push(`**Status:** ${item.status}`)
    if (item.priority) lines.push(`**Priority:** ${item.priority}`)
    if (item.assignee) lines.push(`**Assignee:** ${item.assignee}`)
    if (item.test_file) lines.push(`**Test File:** ${item.test_file}`)
    if (item.file) lines.push(`**File:** ${item.file}`)
    if (item.milestone) lines.push(`**Milestone:** ${item.milestone}`)
    if (item.module) lines.push(`**Module:** ${item.module}`)
    if (item.tags?.length) lines.push(`**Tags:** ${item.tags.join(', ')}`)
    if (item.depends_on?.length) lines.push(`**Depends on:** ${item.depends_on.join(', ')}`)
    lines.push('')
    lines.push(item.content)
    lines.push('')
  }
  return lines.join('\n')
}

async function triggerDriftCheck() {
  try {
    const resp = await authFetch(`/api/forge/specs/${encodeURIComponent(project.value)}/drift-check`, {
      method: 'POST',
    })
    const data = await resp.json()
    alert(`Drift check: ${data.drift_detected ? 'DRIFT DETECTED' : 'No drift detected'} (${data.sections_checked} sections checked)`)
  } catch {
    alert('Drift check failed.')
  }
}

async function rebuildRelations() {
  await apiRebuildRelations(project.value)
  if (error.value) {
    alert('Rebuild failed: ' + error.value)
  } else {
    alert('Relations rebuilt successfully.')
  }
}

onMounted(() => {
  if (project.value && project.value !== 'unknown') {
    loadDocument(project.value)
    fetchOverview()
  }
})

watch(project, (newVal) => {
  if (newVal && newVal !== 'unknown') {
    loadDocument(newVal)
    fetchOverview()
  }
})
</script>

<style scoped>
.specs-view {
  display: flex;
  flex-direction: column;
  height: 100%;
}

.specs-body {
  display: flex;
  flex: 1;
  overflow: hidden;
}

/* ─── Sidebar ─────────────────────────────────────────────── */

.section-nav {
  width: 220px;
  min-width: 220px;
  border-right: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.02);
  display: flex;
  flex-direction: column;
  overflow-y: auto;
  transition: width 0.2s ease, min-width 0.2s ease;
}

.section-nav.collapsed {
  width: 0;
  min-width: 0;
  padding: 0;
  border-right: none;
  overflow: hidden;
}

.section-nav-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  height: 48px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--af-border);
}

.section-nav-title {
  font-size: 0.95rem;
  font-weight: 500;
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  line-height: 1;
  flex: 1;
}

.section-nav-collapse-btn {
  background: none;
  border: none;
  color: var(--af-muted);
  cursor: pointer;
  padding: 0.2rem;
  border-radius: 4px;
}

.section-nav-collapse-btn:hover {
  color: var(--af-fg);
  background: hsl(var(--muted-foreground) / 0.08);
}

.overview-entry {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.6rem 1rem;
  cursor: pointer;
  border-left: 3px solid transparent;
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--af-fg);
  transition: background 0.12s;
  border-bottom: 1px solid var(--af-border);
}

.overview-entry:hover {
  background: hsl(var(--muted-foreground) / 0.05);
}

.overview-entry.active {
  background: hsl(var(--primary) / 0.06);
  border-left-color: hsl(var(--primary));
}

.section-nav-item {
  padding: 0.6rem 1rem;
  cursor: pointer;
  border-left: 3px solid transparent;
  transition: background 0.12s;
}

.section-nav-item:hover {
  background: hsl(var(--muted-foreground) / 0.05);
}

.section-nav-item.active {
  background: hsl(var(--primary) / 0.06);
  border-left-color: hsl(var(--primary));
}

.section-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.2rem;
}

.section-name {
  font-size: 0.88rem;
  font-weight: 500;
  color: var(--af-fg);
}

.section-meta {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.section-count {
  font-size: 0.78rem;
  color: var(--af-muted);
}

/* ─── Module Accordion ────────────────────────────────────── */

.outline-nav-item .section-top {
  gap: 0.4rem;
  justify-content: flex-start;
}

.outline-nav-item .section-top svg {
  color: var(--af-muted);
  flex-shrink: 0;
}

.module-accordion {
  flex: 1;
  overflow-y: auto;
}

.module-drawer {
  border-bottom: 1px solid var(--af-border);
}

.module-drawer:last-child {
  border-bottom: none;
}

.module-summary {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.55rem 0.85rem;
  font-size: 0.82rem;
  font-weight: 600;
  color: var(--af-fg);
  cursor: pointer;
  user-select: none;
  list-style: none;
  background: hsl(var(--muted-foreground) / 0.02);
}

.module-summary::-webkit-details-marker {
  display: none;
}

.module-summary::before {
  content: '▸';
  font-size: 0.7rem;
  color: var(--af-muted);
  transition: transform 0.15s;
  display: inline-block;
  margin-right: 0.35rem;
}

.module-drawer[open] .module-summary::before {
  transform: rotate(90deg);
}

.module-name {
  flex: 1;
  text-transform: capitalize;
}

.module-count {
  font-size: 0.75rem;
  font-weight: 500;
  color: var(--af-muted);
  background: hsl(var(--muted-foreground) / 0.08);
  padding: 0.1rem 0.4rem;
  border-radius: 4px;
}

.type-nav-item {
  padding: 0.45rem 0.85rem 0.45rem 1.6rem;
  border-left: 3px solid transparent;
}

.type-nav-item .section-name {
  font-size: 0.82rem;
  color: var(--af-fg);
}

.type-nav-item .section-count {
  font-size: 0.72rem;
}

/* ─── Filter Drawers ──────────────────────────────────────── */

.filter-drawer {
  border-bottom: 1px solid var(--af-border);
}

.filter-drawer:last-child {
  border-bottom: none;
}

.filter-drawer summary {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.6rem 1rem;
  font-size: 0.72rem;
  font-weight: 600;
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  cursor: pointer;
  user-select: none;
  list-style: none;
}

.filter-drawer summary::-webkit-details-marker {
  display: none;
}

.filter-drawer summary::before {
  content: '▸';
  font-size: 0.7rem;
  transition: transform 0.15s;
  display: inline-block;
}

.filter-drawer[open] summary::before {
  transform: rotate(90deg);
}

.filter-drawer-body {
  padding-bottom: 0.75rem;
}

.filter-drawer-body.pills {
  display: flex;
  flex-wrap: wrap;
  gap: 0.35rem;
  padding: 0 0.75rem 0.75rem;
}

.filter-pill {
  display: inline-flex;
  align-items: center;
  padding: 0.25rem 0.55rem;
  font-size: 0.78rem;
  border-radius: 6px;
  border: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.04);
  color: var(--af-muted);
  cursor: pointer;
  transition: all 0.12s;
  font-family: inherit;
}

.filter-pill:hover {
  border-color: hsl(var(--primary) / 0.3);
  color: var(--af-fg);
}

.filter-pill.active {
  background: hsl(var(--primary) / 0.1);
  border-color: hsl(var(--primary) / 0.35);
  color: hsl(var(--primary));
}

/* ─── Content Pane ────────────────────────────────────────── */

.section-editor {
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
  gap: 0.75rem;
  flex: 1;
  min-width: 0;
}

.section-nav-toggle-btn {
  background: none;
  border: none;
  color: var(--af-muted);
  cursor: pointer;
  padding: 0.3rem;
  border-radius: 4px;
  flex-shrink: 0;
}

.section-nav-toggle-btn:hover {
  color: var(--af-fg);
  background: hsl(var(--muted-foreground) / 0.08);
}

.header-breadcrumb {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.92rem;
  font-weight: 600;
  color: var(--af-fg);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.breadcrumb-project {
  color: var(--af-fg);
}

.breadcrumb-module {
  color: hsl(var(--primary));
  text-transform: capitalize;
}

.breadcrumb-sep {
  color: var(--af-muted);
  font-weight: 400;
}

.breadcrumb-section {
  color: var(--af-fg);
}

.header-search {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  width: 100%;
  max-width: 240px;
  padding: 0.35rem 0.75rem;
  background: hsl(var(--muted-foreground) / 0.06);
  border: 1px solid hsl(var(--muted-foreground) / 0.12);
  border-radius: 6px;
  color: var(--af-muted);
  transition: border-color 0.15s, background 0.15s;
  flex-shrink: 0;
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

.specs-actions {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.specs-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.4rem 0.7rem;
  font-size: 0.83rem;
  border-radius: 6px;
  border: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.04);
  color: var(--af-fg);
  cursor: pointer;
  transition: all 0.15s;
  white-space: nowrap;
}

.specs-btn:hover {
  background: hsl(var(--muted-foreground) / 0.08);
  border-color: hsl(var(--primary) / 0.3);
}

.editor-scroll {
  flex: 1;
  overflow-y: auto;
  padding: 1rem;
}

.editor-pane {
  max-width: 960px;
  margin: 0 auto;
}

.editor-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 0.75rem;
  padding-bottom: 0.5rem;
  border-bottom: 1px solid var(--af-border);
}

.editor-header h3 {
  font-size: 1.18rem;
  font-weight: 700;
  color: var(--af-fg);
  margin: 0;
}

.editor-header-right {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.editor-badges {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.section-toolbar {
  margin-bottom: 0.75rem;
  display: flex;
  align-items: center;
  gap: 0.75rem;
  flex-wrap: wrap;
}

.add-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.4rem 0.7rem;
  font-size: 0.88rem;
  border-radius: 6px;
  border: 1px dashed var(--af-border);
  background: transparent;
  color: var(--af-muted);
  cursor: pointer;
}

.add-btn:hover {
  color: hsl(var(--primary));
  border-color: hsl(var(--primary) / 0.4);
  background: hsl(var(--primary) / 0.04);
}

.tag-filter-bar {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  flex-wrap: wrap;
}

.tag-filter-icon {
  color: var(--af-muted);
  flex-shrink: 0;
}

.tag-filter-chip {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  padding: 0.25rem 0.55rem;
  font-size: 0.78rem;
  border-radius: 6px;
  border: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.04);
  color: var(--af-muted);
  cursor: pointer;
  transition: all 0.12s;
}

.tag-filter-chip:hover {
  border-color: hsl(var(--primary) / 0.3);
  color: var(--af-fg);
}

.tag-filter-chip.active {
  background: hsl(var(--primary) / 0.1);
  border-color: hsl(var(--primary) / 0.35);
  color: hsl(var(--primary));
}

.tag-filter-clear {
  display: inline-flex;
  align-items: center;
  gap: 0.15rem;
  padding: 0.25rem 0.45rem;
  font-size: 0.75rem;
  border-radius: 6px;
  border: 1px solid transparent;
  background: transparent;
  color: var(--af-muted);
  cursor: pointer;
  transition: all 0.12s;
}

.tag-filter-clear:hover {
  color: hsl(var(--destructive));
  background: hsl(var(--destructive) / 0.06);
}

.editor-empty {
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

/* ─── Mobile Responsive ───────────────────────────────────────────────────── */

@media (max-width: 768px) {
  .section-nav {
    position: fixed;
    left: 0;
    top: 0;
    bottom: 0;
    z-index: 50;
    background: var(--af-bg);
    box-shadow: 2px 0 8px rgba(0, 0, 0, 0.1);
  }

  .section-nav.collapsed {
    width: 0;
    min-width: 0;
    padding: 0;
    overflow: hidden;
  }

  .content-header {
    padding: 0.5rem 0.75rem;
  }

  .header-search {
    min-width: 120px;
  }

  .editor-pane {
    padding: 0 0.5rem;
  }
}

/* Flash animation for jump-to-item */
@keyframes flash-highlight {
  0% { background: hsl(48 100% 60% / 0.35); }
  100% { background: transparent; }
}

:deep(.spec-item-row.flash) {
  animation: flash-highlight 1.5s ease-out;
}

</style>
