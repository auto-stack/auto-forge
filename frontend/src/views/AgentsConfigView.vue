<template>
  <div class="agents-config-view">
    <div class="agents-header">
      <h2>{{ t('agents.title') }}</h2>
      <div class="header-actions">
        <button class="btn-secondary" @click="handleResetDefaults">{{ t('common.reset') }}</button>
        <button class="btn-primary" @click="startCreate">
          <Plus :size="14" /> {{ t('agents.createAgent') }}
        </button>
      </div>
    </div>

    <div v-if="loading" class="agents-empty">{{ t('agents.loading') }}</div>

    <div v-else-if="!configs.length" class="agents-empty">
      <Users :size="48" />
      <p>{{ t('agents.noAgents') }}</p>
    </div>

    <div v-else class="agents-grid">
      <div
        v-for="agent in configs"
        :key="agent.id"
        class="agent-card"
        :class="{ default: agent.is_default, editing: editingId === agent.id }"
        @click="startEdit(agent)"
      >
        <AgentAvatar :profession-id="agent.profession_id" :name="agent.name" :agent-id="agent.id" size="lg" />
        <div class="card-name">{{ agent.name }}</div>
        <div class="card-badges">
          <span class="badge profession-badge">{{ agent.profession_id }}</span>
          <span class="badge tier-badge" :class="agent.model_tier">{{ tierLabel(agent.model_tier) }}</span>
          <span v-if="agent.is_default" class="badge default-badge">{{ t('common.default') }}</span>
        </div>
        <div v-if="agent.equipped_skills?.length" class="card-skills">
          <span v-for="sid in agent.equipped_skills" :key="sid" class="skill-chip">{{ skillName(sid) }}</span>
        </div>
        <div class="card-soul-preview">
          {{ getSoulPreview(agent.soul_id) }}
        </div>
        <div class="card-actions">
          <button class="btn-small" @click.stop="startEdit(agent)">{{ t('common.edit') }}</button>
          <button v-if="!agent.is_default" class="btn-small btn-small" @click.stop="handleDelete(agent.id)">
            <Trash2 :size="12" />
          </button>
        </div>
      </div>
    </div>

    <!-- Edit Panel -->
    <div v-if="editing" class="edit-overlay" @click.self="cancelEdit">
      <div class="edit-panel">
        <div class="edit-header">
          <h3>{{ isNew ? t('agents.createAgent') : editing.name }}</h3>
          <button class="btn-close" @click="cancelEdit"><X :size="16" /></button>
        </div>

        <div class="edit-form">
          <!-- Avatar Editor -->
          <div class="avatar-editor">
            <AgentAvatar
              :profession-id="editing.profession_id"
              :name="editing.name"
              :agent-id="editing.id"
              :image-url="editing.avatar_url"
              size="lg"
            />
            <div class="avatar-actions">
              <input
                ref="avatarInput"
                type="file"
                accept="image/png,image/jpeg,image/gif,image/webp"
                style="display: none"
                @change="handleAvatarUpload"
              />
              <button class="btn-small" @click="avatarInput?.click()">
                <Upload :size="12" /> {{ t('common.upload') }}
              </button>
              <button class="btn-small" :disabled="generatingAvatar" @click="handleAvatarGenerate">
                <Sparkles :size="12" /> {{ generatingAvatar ? t('common.generating') : t('common.generate') }}
              </button>
              <button v-if="editing.avatar_url" class="btn-small btn-danger" @click="handleAvatarRemove">
                <Trash2 :size="12" /> {{ t('common.remove') }}
              </button>
            </div>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>{{ t('agents.agentName') }}</label>
              <input v-model="editing.name" class="form-input" :placeholder="t('agents.agentNamePlaceholder')" />
            </div>
            <div class="form-group">
              <label>{{ t('agents.profession') }}</label>
              <select v-model="editing.profession_id" class="form-select" :disabled="!isNew">
                <option v-for="p in professions" :key="p.id" :value="p.id">
                  {{ professionEmoji(p.id) }} {{ p.name }}
                </option>
              </select>
            </div>
          </div>

          <div class="form-group">
            <label>{{ t('agents.soul') }}</label>
            <textarea v-model="soulMarkdown" class="form-textarea" rows="6" :placeholder="t('agents.soulPlaceholder')" />
            <div class="form-hint">{{ t('agents.soulHint') }}</div>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>{{ t('agents.apiSource') }}</label>
              <select v-model="editing.api_source_id" class="form-select">
                <option v-for="s in apiSources" :key="s.id" :value="s.id">
                  {{ s.name }}
                </option>
              </select>
            </div>
            <div class="form-group">
              <label>{{ t('agents.modelTier') }}</label>
              <div class="tier-selector">
                <button
                  v-for="t in tiers"
                  :key="t.value"
                  class="tier-option"
                  :class="{ active: editing.model_tier === t.value, [t.value]: true }"
                  @click="editing.model_tier = t.value"
                >
                  <span class="tier-bars">
                    <span v-for="n in t.bars" :key="n" class="tier-bar" />
                  </span>
                  <span class="tier-label">{{ t.label }}</span>
                </button>
              </div>
            </div>
          </div>

          <div class="form-group">
            <label>{{ t('agents.equippedSkills') }}</label>
            <div class="skills-selector">
              <label
                v-for="skill in skills"
                :key="skill.id"
                class="skill-checkbox"
                :class="{ checked: editing.equipped_skills?.includes(skill.id) }"
              >
                <input
                  type="checkbox"
                  :checked="editing.equipped_skills?.includes(skill.id)"
                  @change="toggleSkill(skill.id)"
                />
                <span class="skill-check-name">{{ skill.name }}</span>
                <span class="skill-check-desc">{{ skill.granted_tools.join(', ') }}</span>
              </label>
            </div>
          </div>

          <details class="advanced-section">
            <summary>{{ t('common.advanced') }}</summary>
            <div class="form-row">
              <div class="form-group">
                <label>{{ t('agents.temperature', { value: editing.temperature.toFixed(1) }) }}</label>
                <input v-model.number="editing.temperature" type="range" min="0" max="1" step="0.1" class="form-range" />
              </div>
              <div class="form-group">
                <label>{{ t('agents.maxTokens') }}</label>
                <input v-model.number="editing.max_tokens" type="number" class="form-input" min="256" max="32768" step="256" />
              </div>
              <div v-if="editing.model_tier === 'pro' || editing.model_tier === 'max'" class="form-group">
                <label>{{ t('agents.reasoningBudget') }}</label>
                <input v-model.number="editing.reasoning_budget" type="number" class="form-input" min="0" max="16384" step="512" />
              </div>
            </div>
            <div class="form-row">
              <div class="form-group">
                <label class="toggle-label">
                  <input v-model="editing.thinking_enabled" type="checkbox" />
                  <span>{{ t('agents.thinkingMode') }}</span>
                </label>
                <div class="form-hint">{{ t('agents.thinkingHint') }}</div>
              </div>
              <div v-if="editing.thinking_enabled" class="form-group">
                <label>{{ t('agents.thinkingBudget', { value: editing.thinking_budget ?? 0 }) }}</label>
                <input
                  v-model.number="editing.thinking_budget"
                  type="range"
                  min="512"
                  max="4096"
                  step="512"
                  class="form-range"
                />
              </div>
            </div>
          </details>

          <div class="edit-footer">
            <button class="btn-primary" @click="handleSave" :disabled="saving">
              {{ saving ? t('common.saving') : t('common.save') }}
            </button>
            <button class="btn-secondary" @click="cancelEdit">{{ t('common.cancel') }}</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ref, onMounted, computed, watch } from 'vue'
import { Plus, X, Trash2, Users, Upload, Sparkles } from 'lucide-vue-next'
import { useAgentConfigs, type AgentConfigDto } from '@/composables/useAgentConfigs'
import AgentAvatar from '@/components/AgentAvatar.vue'
import { useApiSources, type ApiSource } from '@/composables/useApiSources'
import { useSouls } from '@/composables/useSouls'
import { useSkills } from '@/composables/useSkills'

const { t } = useI18n()
const {
  configs, loading, error,
  loadConfigs, createConfig, updateConfig, deleteConfig, resetDefaults,
} = useAgentConfigs()
const { sources: apiSources, loadSources } = useApiSources()
const { souls, soulMap, loadSouls, getSoulMarkdown } = useSouls()
const { skills, loadSkills: loadSkillsList } = useSkills()

const editing = ref<AgentConfigDto | null>(null)
const editingId = ref<string | null>(null)
const isNew = ref(false)
const saving = ref(false)
const soulMarkdown = ref('')
const avatarInput = ref<HTMLInputElement | null>(null)
const generatingAvatar = ref(false)

const professions = [
  { id: 'assistant', name: 'Assistant' },
  { id: 'advisor', name: 'Advisor' },
  { id: 'architect', name: 'Architect' },
  { id: 'planner', name: 'Planner' },
  { id: 'coder', name: 'Coder' },
  { id: 'tester', name: 'Tester' },
  { id: 'reviewer', name: 'Reviewer' },
  { id: 'documenter', name: 'Documenter' },
  { id: 'gofer', name: 'Gofer' },
]

const tiers = [
  { value: 'min' as const, label: 'Min', bars: 1 },
  { value: 'lite' as const, label: 'Lite', bars: 1 },
  { value: 'mid' as const, label: 'Mid', bars: 2 },
  { value: 'pro' as const, label: 'Pro', bars: 3 },
  { value: 'max' as const, label: 'Max', bars: 3 },
]

function tierLabel(tier: string): string {
  const map: Record<string, string> = { min: 'Min', lite: 'Lite', mid: 'Mid', pro: 'Pro', max: 'Max' }
  return map[tier] || tier
}

const professionEmoji = (id: string) => {
  const map: Record<string, string> = {
    assistant: '🎯', advisor: '🔍', architect: '🏗️', planner: '📅',
    coder: '💻', tester: '🧪', reviewer: '📝', documenter: '📊',
    gofer: '🔎',
  }
  return map[id] || '🤖'
}

const soulPreviews: Record<string, string> = {
  assistant: 'Routes requests, answers questions, and dispatches work.',
  advisor: 'Discovers goals, analyzes requirements, and drives discovery.',
  architect: 'Designs systems with simplicity and stability in mind.',
  planner: 'Breaks goals into phases with clear dependencies.',
  coder: 'Implements designs following plans and tests.',
  tester: 'Generates and runs tests to verify correctness.',
  reviewer: 'Audits work against goals and quality standards.',
  gofer: 'Fetches facts and gathers information on request.',
  documenter: 'Compiles reports and generates documentation.',
}

function getSoulPreview(soulId: string): string {
  const md = getSoulMarkdown(soulId)
  if (md) {
    const personality = md.match(/^##\s*Personality\s*\n([\s\S]*?)(?=\n##|\n$)/)?.[1]?.trim()
    if (personality) return personality.slice(0, 120)
    const firstLine = md.split('\n').find(l => l.trim() && !l.startsWith('#'))?.trim()
    if (firstLine) return firstLine.slice(0, 120)
  }
  return soulPreviews[soulId] || 'Custom agent soul.'
}

function startEdit(agent: AgentConfigDto) {
  editing.value = {
    ...agent,
    thinking_enabled: agent.thinking_enabled ?? false,
    thinking_budget: agent.thinking_budget ?? 2048,
  }
  editingId.value = agent.id
  isNew.value = false
  soulMarkdown.value = getSoulMarkdown(agent.soul_id)
}

// Re-populate soul markdown when souls load (handles race condition)
watch([() => editing.value?.soul_id, soulMap], () => {
  if (editing.value) {
    soulMarkdown.value = getSoulMarkdown(editing.value.soul_id)
  }
}, { immediate: true })

function startCreate() {
  editing.value = {
    id: `agent-${Date.now()}`,
    name: '',
    profession_id: 'coder',
    soul_id: 'coder',
    api_source_id: apiSources.value[0]?.id ?? '',
    model_tier: 'mid',
    is_default: false,
    temperature: 0.3,
    max_tokens: 4096,
    reasoning_budget: null,
    thinking_enabled: true,
    thinking_budget: 2048,
    equipped_skills: [],
  }
  editingId.value = null
  isNew.value = true
  soulMarkdown.value = ''
}

function cancelEdit() {
  editing.value = null
  editingId.value = null
}

async function handleSave() {
  if (!editing.value) return
  saving.value = true
  if (isNew.value) {
    const ok = await createConfig(editing.value)
    if (ok) cancelEdit()
  } else {
    const ok = await updateConfig(editing.value.id, editing.value)
    if (ok) cancelEdit()
  }
  saving.value = false
}

async function handleDelete(id: string) {
  if (!confirm('Delete this agent?')) return
  await deleteConfig(id)
}

async function handleAvatarUpload(e: Event) {
  const input = e.target as HTMLInputElement
  const file = input.files?.[0]
  if (!file || !editing.value) return

  const formData = new FormData()
  formData.append('file', file)

  try {
    const resp = await fetch(`/api/forge/config/agents/${editing.value.id}/avatar`, {
      method: 'POST',
      body: formData,
    })
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
    const data = await resp.json()
    editing.value.avatar_url = data.avatar_url
    // Also update in the global configs list so the grid updates immediately
    const idx = configs.value.findIndex(c => c.id === editing.value!.id)
    if (idx >= 0) configs.value[idx].avatar_url = data.avatar_url
  } catch (err) {
    alert('Upload failed: ' + (err instanceof Error ? err.message : String(err)))
  } finally {
    input.value = ''
  }
}

async function handleAvatarGenerate() {
  if (!editing.value) return
  generatingAvatar.value = true
  try {
    const resp = await fetch(`/api/forge/config/agents/${editing.value.id}/avatar/generate`, {
      method: 'POST',
    })
    if (!resp.ok) throw new Error(`HTTP ${resp.status}`)
    const data = await resp.json()
    editing.value.avatar_url = data.avatar_url
    const idx = configs.value.findIndex(c => c.id === editing.value!.id)
    if (idx >= 0) configs.value[idx].avatar_url = data.avatar_url
  } catch (err) {
    alert('Generation failed: ' + (err instanceof Error ? err.message : String(err)))
  } finally {
    generatingAvatar.value = false
  }
}

function handleAvatarRemove() {
  if (!editing.value) return
  editing.value.avatar_url = undefined
  const idx = configs.value.findIndex(c => c.id === editing.value!.id)
  if (idx >= 0) configs.value[idx].avatar_url = undefined
}

async function handleResetDefaults() {
  if (!confirm('Reset to 8 default agents? Custom agents will be kept.')) return
  await resetDefaults()
}

onMounted(() => {
  loadConfigs()
  loadSources()
  loadSouls()
  loadSkillsList()
})

function skillName(id: string): string {
  return skills.value.find(s => s.id === id)?.name || id
}

function toggleSkill(skillId: string) {
  if (!editing.value) return
  const current = editing.value.equipped_skills || []
  if (current.includes(skillId)) {
    editing.value.equipped_skills = current.filter(id => id !== skillId)
  } else {
    editing.value.equipped_skills = [...current, skillId]
  }
}
</script>

<style scoped>
.agents-config-view {
  height: 100%;
  overflow-y: auto;
  padding: 1.5rem;
}

.agents-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  height: 48px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--af-border);
  gap: 1rem;
}

.agents-header h2 {
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
  gap: 0.5rem;
}

.agents-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 50%;
  color: var(--af-muted);
  gap: 0.75rem;
  font-size: 0.93rem;
}

/* Grid */
.agents-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1rem;
  max-width: 1200px;
  margin: 1rem auto 0;
  align-items: stretch;
}

.agent-card {
  background: var(--af-card);
  border: 1px solid var(--af-border);
  border-radius: 10px;
  padding: 1.25rem;
  cursor: pointer;
  transition: all 0.2s;
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  height: 100%;
}

.agent-card:hover {
  border-color: hsl(var(--primary) / 0.3);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

.agent-card.default {
  border-left: 3px solid var(--af-primary);
}

.agent-card.editing {
  border-color: var(--af-primary);
  box-shadow: 0 0 0 2px hsl(var(--primary) / 0.15);
}

.card-name {
  font-size: 1.03rem;
  font-weight: 600;
}

.card-badges {
  display: flex;
  gap: 0.35rem;
  flex-wrap: wrap;
}

.badge {
  font-size: 0.73rem;
  font-weight: 600;
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.profession-badge {
  background: hsl(var(--primary) / 0.1);
  color: var(--af-primary);
}

.tier-badge.min { background: hsl(140 60% 40% / 0.12); color: hsl(140 60% 35%); }
.tier-badge.lite { background: hsl(150 55% 42% / 0.12); color: hsl(150 55% 37%); }
.tier-badge.mid { background: hsl(210 60% 50% / 0.12); color: hsl(210 60% 45%); }
.tier-badge.pro { background: hsl(260 55% 52% / 0.12); color: hsl(260 55% 47%); }
.tier-badge.max { background: hsl(280 50% 50% / 0.12); color: hsl(280 50% 45%); }

.default-badge {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-muted);
}

.card-soul-preview {
  font-size: 0.83rem;
  color: var(--af-muted);
  line-height: 1.4;
}

.card-skills {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}

.skill-chip {
  font-size: 0.72rem;
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
  background: hsl(var(--primary) / 0.08);
  color: var(--af-primary);
}

.skills-selector {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  max-height: 160px;
  overflow-y: auto;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  padding: 0.4rem;
}

.skill-checkbox {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.3rem 0.4rem;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.1s;
  font-size: 0.88rem;
}

.skill-checkbox:hover {
  background: hsl(var(--muted-foreground) / 0.04);
}

.skill-checkbox.checked {
  background: hsl(var(--primary) / 0.06);
}

.skill-checkbox input {
  cursor: pointer;
}

.skill-check-name {
  font-weight: 500;
}

.skill-check-desc {
  margin-left: auto;
  font-size: 0.75rem;
  color: var(--af-muted);
}

.card-actions {
  display: flex;
  gap: 0.3rem;
  margin-top: auto;
  padding-top: 0.3rem;
}

/* Edit Panel */
.edit-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 50;
}

.edit-panel {
  background: var(--af-card);
  border: 1px solid var(--af-border);
  border-radius: 12px;
  width: 560px;
  max-height: 80vh;
  overflow-y: auto;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.12);
}

.edit-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--af-border);
}

.edit-header h3 {
  font-size: 1rem;
  font-weight: 600;
}

.btn-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: var(--af-muted);
  cursor: pointer;
}

.btn-close:hover { background: hsl(var(--muted-foreground) / 0.08); }

.edit-form {
  padding: 1.25rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.form-row {
  display: flex;
  gap: 1rem;
}

.form-row .form-group {
  flex: 1;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.form-group label {
  font-size: 0.86rem;
  font-weight: 500;
}

.form-input, .form-select, .form-textarea {
  padding: 0.45rem 0.6rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: var(--af-bg);
  color: var(--af-fg);
  font-size: 0.9rem;
  outline: none;
  transition: border-color 0.15s;
}

.form-input:focus, .form-select:focus, .form-textarea:focus {
  border-color: var(--af-primary);
}

.form-textarea {
  font-family: monospace;
  resize: vertical;
}

.form-hint {
  font-size: 0.78rem;
  color: var(--af-muted);
}

.form-range {
  width: 100%;
}

.toggle-label {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  cursor: pointer;
  font-weight: 500;
  font-size: 0.88rem;
}

.toggle-label input[type="checkbox"] {
  width: 1rem;
  height: 1rem;
  accent-color: var(--af-primary);
}

/* Tier selector */
.tier-selector {
  display: flex;
  gap: 0.4rem;
}

.tier-option {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.25rem;
  padding: 0.5rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: var(--af-bg);
  cursor: pointer;
  transition: all 0.15s;
}

.tier-option:hover {
  border-color: hsl(var(--primary) / 0.3);
}

.tier-option.active.min {
  border-color: hsl(140 60% 35%);
  background: hsl(140 60% 40% / 0.08);
}

.tier-option.active.lite {
  border-color: hsl(150 55% 37%);
  background: hsl(150 55% 42% / 0.08);
}

.tier-option.active.mid {
  border-color: hsl(210 60% 45%);
  background: hsl(210 60% 50% / 0.08);
}

.tier-option.active.pro {
  border-color: hsl(260 55% 47%);
  background: hsl(260 55% 52% / 0.08);
}

.tier-option.active.max {
  border-color: hsl(280 50% 45%);
  background: hsl(280 50% 50% / 0.08);
}

.tier-bars {
  display: flex;
  gap: 2px;
}

.tier-bar {
  width: 4px;
  height: 14px;
  border-radius: 1px;
  background: var(--af-muted);
}

.tier-option.active.min .tier-bar { background: hsl(140 60% 35%); }
.tier-option.active.lite .tier-bar { background: hsl(150 55% 37%); }
.tier-option.active.mid .tier-bar { background: hsl(210 60% 45%); }
.tier-option.active.pro .tier-bar { background: hsl(260 55% 47%); }
.tier-option.active.max .tier-bar { background: hsl(280 50% 45%); }

.tier-label {
  font-size: 0.78rem;
  font-weight: 500;
}

/* Advanced */
.advanced-section {
  border: 1px solid var(--af-border);
  border-radius: 6px;
  padding: 0.5rem 0.75rem;
}

.advanced-section summary {
  font-size: 0.86rem;
  font-weight: 500;
  cursor: pointer;
  color: var(--af-muted);
}

.edit-footer {
  display: flex;
  gap: 0.5rem;
  justify-content: flex-end;
  padding-top: 0.5rem;
  border-top: 1px solid var(--af-border);
}

/* Buttons */
.btn-primary, .btn-secondary, .btn-small {
  border: none;
  border-radius: 6px;
  cursor: pointer;
  font-size: 0.86rem;
  transition: all 0.15s;
}

.btn-primary {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.4rem 0.8rem;
  background: var(--af-primary);
  color: #fff;
  font-weight: 500;
}

.btn-primary:disabled { opacity: 0.6; }

.btn-secondary {
  padding: 0.4rem 0.8rem;
  background: transparent;
  border: 1px solid var(--af-border);
  color: var(--af-fg);
}

.btn-small {
  display: flex;
  align-items: center;
  gap: 0.2rem;
  padding: 0.25rem 0.5rem;
  background: transparent;
  border: 1px solid var(--af-border);
  color: var(--af-muted);
}

.btn-small:hover {
  background: hsl(var(--muted-foreground) / 0.06);
  color: var(--af-fg);
}

.btn-small.btn-danger {
  color: hsl(var(--af-error));
  border-color: hsl(var(--af-error) / 0.3);
}

.btn-small.btn-danger:hover {
  background: hsl(var(--af-error) / 0.08);
}

/* Avatar Editor */
.avatar-editor {
  display: flex;
  align-items: center;
  gap: 1rem;
  padding: 0.75rem;
  background: hsl(var(--muted-foreground) / 0.04);
  border: 1px solid var(--af-border);
  border-radius: 10px;
}

.avatar-actions {
  display: flex;
  gap: 0.4rem;
  flex-wrap: wrap;
}
</style>
