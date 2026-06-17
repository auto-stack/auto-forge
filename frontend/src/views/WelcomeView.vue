<template>
  <div class="welcome-view">
    <div class="welcome-card">
      <!-- STEP: Welcome / directory selection -->
      <template v-if="wizardStep === 'welcome'">
        <div class="welcome-logo">
          <Flame :size="64" />
        </div>
        <h1 class="welcome-title">{{ t('welcome.title') }}</h1>
        <p class="welcome-subtitle">{{ t('welcome.subtitle') }}</p>

        <div class="open-section">
          <div class="path-input-row">
            <button class="btn-browse" @click="handleBrowse" :title="t('welcome.browseTitle')">
              <FolderOpen :size="14" />
            </button>
            <input
              v-model="projectPath"
              type="text"
              class="path-input"
              :placeholder="t('welcome.pathPlaceholder')"
              @keydown.enter="handleOpen"
            />
            <button class="btn-open" :disabled="!projectPath.trim() || isLoading" @click="handleOpen">
              <FolderOpen :size="14" />
              <span>{{ isLoading ? t('welcome.opening') : t('welcome.openButton') }}</span>
            </button>
          </div>
          <div v-if="browseEntries.length > 0" class="browse-list">
            <button
              v-for="entry in browseEntries"
              :key="entry.path"
              class="browse-entry"
              @click="projectPath = entry.path"
            >
              <Folder :size="14" />
              <span>{{ entry.name }}</span>
            </button>
          </div>
          <p v-if="error" class="error-text">{{ error }}</p>
        </div>

        <div v-if="recentProjects.length > 0" class="recent-section">
          <h3 class="recent-title">{{ t('welcome.recentProjects') }}</h3>
          <button
            v-for="rp in recentProjects"
            :key="rp.path"
            class="recent-item"
            @click="handleRecentProject(rp.path)"
          >
            <Folder :size="16" />
            <div class="recent-info">
              <span class="recent-name">{{ rp.name }}</span>
              <span class="recent-path">{{ rp.path }}</span>
            </div>
          </button>
        </div>
      </template>

      <!-- STEP: Empty directory confirmation -->
      <template v-if="wizardStep === 'confirm'">
        <div class="wizard-logo">
          <FolderOpen :size="48" />
        </div>
        <h2 class="wizard-title">{{ t('welcome.emptyDirTitle') }}</h2>
        <p class="wizard-subtitle">{{ t('welcome.emptyDirMessage') }}</p>
        <div class="wizard-actions">
          <button class="btn-secondary" @click="handleCancel">
            {{ t('welcome.no') }}
          </button>
          <button class="btn-primary" @click="wizardStep = 'describe'">
            {{ t('welcome.yes') }}
          </button>
        </div>
      </template>

      <!-- STEP: Project description -->
      <template v-if="wizardStep === 'describe'">
        <div class="wizard-logo">
          <Flame :size="48" />
        </div>
        <h2 class="wizard-title">{{ t('welcome.projectDescriptionPrompt') }}</h2>
        <textarea
          v-model="projectDescription"
          class="project-description-input"
          :placeholder="t('welcome.projectDescriptionPlaceholder')"
          rows="3"
          @keydown.enter.prevent="handleStartProject"
        />
        <div class="wizard-actions">
          <button class="btn-secondary" @click="wizardStep = 'confirm'">
            {{ t('welcome.back') }}
          </button>
          <button
            class="btn-primary"
            :disabled="!projectDescription.trim() || isLoading"
            @click="handleStartProject"
          >
            {{ isLoading ? t('welcome.opening') : t('welcome.startProject') }}
          </button>
        </div>
      </template>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, watch, onMounted, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'
import { Flame, FolderOpen, Folder } from 'lucide-vue-next'
import { useProject } from '@/composables/useProject'
import { useForge } from '@/composables/useForge'
import { authFetch } from '@/composables/useAuth'

const { t } = useI18n()
const { openProject, isLoading, error, projectInfo, recentProjects, fetchRecentProjects, browseDirectory } = useProject()
const { clearSession, sendMessage } = useForge()

const emit = defineEmits<{
  (e: 'start-chat'): void
}>()

type WizardStep = 'welcome' | 'confirm' | 'describe'

const projectPath = ref('')
const projectDescription = ref('')
const browseEntries = ref<{ name: string; path: string }[]>([])
const wizardStep = ref<WizardStep>('welcome')

async function handleOpen() {
  const path = projectPath.value.trim()
  if (!path) return
  const info = await openProject(path)
  if (info?.is_empty) {
    wizardStep.value = 'confirm'
  }
}

async function handleBrowse() {
  try {
    const resp = await authFetch('/api/forge/project/pick-folder')
    if (!resp.ok) return
    const path: string | null = await resp.json()
    if (path) {
      projectPath.value = path
    }
  } catch { /* dialog failed or cancelled */ }
}

async function handleRecentProject(path: string) {
  projectPath.value = path
  await handleOpen()
}

function handleCancel() {
  projectPath.value = ''
  projectDescription.value = ''
  wizardStep.value = 'welcome'
}

async function handleStartProject() {
  const path = projectPath.value.trim()
  const description = projectDescription.value.trim()
  if (!path || !description) return

  // Ensure project is open and a fresh session is created before sending the first message.
  await clearSession(path)
  await sendMessage(description)

  // Mark the project as no longer empty so App.vue switches from WelcomeView to ChatsView,
  // and tell App.vue to show the chat tab.
  if (projectInfo.value) {
    projectInfo.value.is_empty = false
  }
  emit('start-chat')
}

let browseTimer: ReturnType<typeof setTimeout> | null = null

watch(projectPath, (val) => {
  if (browseTimer) clearTimeout(browseTimer)
  if (!val || val.length < 2) {
    browseEntries.value = []
    return
  }
  browseTimer = setTimeout(async () => {
    try {
      const lastSep = Math.max(val.lastIndexOf('/'), val.lastIndexOf('\\'))
      const parentDir = lastSep > 0 ? val.substring(0, lastSep) : val
      const entries = await browseDirectory(parentDir)
      const lastSeg = val.substring(lastSep + 1).toLowerCase()
      browseEntries.value = lastSeg
        ? entries.filter(e => e.name.toLowerCase().startsWith(lastSeg))
        : entries.slice(0, 20)
    } catch {
      browseEntries.value = []
    }
  }, 300)
})

onMounted(() => {
  fetchRecentProjects()
})
</script>

<style scoped>
.welcome-view {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--af-bg);
  padding: 2rem;
}

.welcome-card {
  width: 100%;
  max-width: 520px;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 1rem;
}

.welcome-logo,
.wizard-logo {
  color: var(--af-primary);
  margin-bottom: 0.5rem;
}

.welcome-title,
.wizard-title {
  font-size: 2.48rem;
  font-weight: 700;
  color: var(--af-fg);
  text-align: center;
}

.wizard-title {
  font-size: 1.6rem;
}

.welcome-subtitle,
.wizard-subtitle {
  font-size: 0.98rem;
  color: var(--af-muted);
  margin-top: -0.5rem;
  text-align: center;
}

.wizard-subtitle {
  margin-top: 0;
  max-width: 420px;
}

.open-section {
  width: 100%;
  margin-top: 1rem;
}

.path-input-row {
  display: flex;
  gap: 0.5rem;
}

.btn-browse,
.btn-open,
.btn-primary,
.btn-secondary {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  padding: 0.6rem 1rem;
  border-radius: 6px;
  font-size: 0.93rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.15s, border-color 0.15s;
  white-space: nowrap;
}

.btn-browse {
  width: 36px;
  height: 36px;
  padding: 0;
  border: 1px solid var(--af-border);
  background: var(--af-card);
  color: var(--af-muted);
  flex-shrink: 0;
}

.btn-browse:hover {
  border-color: var(--af-primary);
  color: var(--af-primary);
}

.btn-open,
.btn-primary {
  background: var(--af-primary);
  color: #fff;
  border: none;
}

.btn-open:hover:not(:disabled),
.btn-primary:hover:not(:disabled) {
  opacity: 0.9;
}

.btn-open:disabled,
.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary {
  background: var(--af-card);
  color: var(--af-fg);
  border: 1px solid var(--af-border);
}

.btn-secondary:hover:not(:disabled) {
  border-color: var(--af-primary);
  color: var(--af-primary);
}

.path-input {
  flex: 1;
  padding: 0.6rem 0.8rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: var(--af-card);
  color: var(--af-fg);
  font-size: 0.93rem;
  font-family: inherit;
  outline: none;
  transition: border-color 0.15s;
}

.path-input:focus {
  border-color: var(--af-primary);
}

.project-description-input {
  width: 100%;
  padding: 0.8rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: var(--af-card);
  color: var(--af-fg);
  font-size: 0.95rem;
  font-family: inherit;
  line-height: 1.5;
  resize: vertical;
  outline: none;
  transition: border-color 0.15s;
}

.project-description-input:focus {
  border-color: var(--af-primary);
}

.browse-list {
  margin-top: 0.4rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  max-height: 180px;
  overflow-y: auto;
  background: var(--af-card);
}

.browse-entry {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.4rem 0.7rem;
  border: none;
  background: transparent;
  color: var(--af-fg);
  font-size: 0.88rem;
  cursor: pointer;
  text-align: left;
  transition: background 0.1s;
}

.browse-entry:hover {
  background: hsl(var(--primary) / 0.06);
  color: var(--af-primary);
}

.error-text {
  margin-top: 0.4rem;
  font-size: 0.88rem;
  color: var(--af-error, #dc2626);
}

.recent-section {
  width: 100%;
  margin-top: 1.5rem;
  border-top: 1px solid var(--af-border);
  padding-top: 1rem;
}

.recent-title {
  font-size: 0.83rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--af-muted);
  margin-bottom: 0.6rem;
}

.recent-item {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  width: 100%;
  padding: 0.5rem 0.6rem;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--af-fg);
  cursor: pointer;
  text-align: left;
  transition: background 0.1s;
}

.recent-item:hover {
  background: hsl(var(--primary) / 0.06);
}

.recent-info {
  display: flex;
  flex-direction: column;
  min-width: 0;
}

.recent-name {
  font-size: 0.93rem;
  font-weight: 500;
}

.recent-path {
  font-size: 0.78rem;
  color: var(--af-muted);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.wizard-actions {
  display: flex;
  gap: 0.75rem;
  margin-top: 0.5rem;
}
</style>
