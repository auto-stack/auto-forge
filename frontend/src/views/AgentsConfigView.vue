<template>
  <div class="agents-config-view">
    <div class="agents-header">
      <h2>Agent Forge</h2>
      <div class="header-actions">
        <button class="btn-secondary" @click="handleResetDefaults">Reset Defaults</button>
        <button class="btn-primary" @click="startCreate">
          <Plus :size="14" /> Create Agent
        </button>
      </div>
    </div>

    <div v-if="loading" class="agents-empty">Loading agents...</div>

    <div v-else-if="!configs.length" class="agents-empty">
      <Users :size="48" />
      <p>No agents configured. Reset defaults to get started.</p>
    </div>

    <div v-else class="agents-grid">
      <div
        v-for="agent in configs"
        :key="agent.id"
        class="agent-card"
        :class="{ default: agent.is_default, editing: editingId === agent.id }"
        @click="startEdit(agent)"
      >
        <div class="card-avatar" :class="agent.profession_id">
          <span class="avatar-emoji">{{ professionEmoji(agent.profession_id) }}</span>
        </div>
        <div class="card-name">{{ agent.name }}</div>
        <div class="card-badges">
          <span class="badge profession-badge">{{ agent.profession_id }}</span>
          <span class="badge tier-badge" :class="agent.model_tier">{{ agent.model_tier }}</span>
          <span v-if="agent.is_default" class="badge default-badge">Default</span>
        </div>
        <div class="card-soul-preview">
          {{ getSoulPreview(agent.soul_id) }}
        </div>
        <div class="card-actions">
          <button class="btn-small" @click.stop="startEdit(agent)">Edit</button>
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
          <h3>{{ isNew ? 'Create Agent' : editing.name }}</h3>
          <button class="btn-close" @click="cancelEdit"><X :size="16" /></button>
        </div>

        <div class="edit-form">
          <div class="form-row">
            <div class="form-group">
              <label>Name</label>
              <input v-model="editing.name" class="form-input" placeholder="Agent name" />
            </div>
            <div class="form-group">
              <label>Profession</label>
              <select v-model="editing.profession_id" class="form-select" :disabled="!isNew">
                <option v-for="p in professions" :key="p.id" :value="p.id">
                  {{ professionEmoji(p.id) }} {{ p.name }}
                </option>
              </select>
            </div>
          </div>

          <div class="form-group">
            <label>Soul</label>
            <textarea v-model="soulMarkdown" class="form-textarea" rows="6" placeholder="Soul markdown..." />
            <div class="form-hint">Defines the agent's personality, values, and working style.</div>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>API Source</label>
              <select v-model="editing.api_source_id" class="form-select">
                <option v-for="s in apiSources" :key="s.id" :value="s.id">
                  {{ s.name }}
                </option>
              </select>
            </div>
            <div class="form-group">
              <label>Model Tier</label>
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

          <details class="advanced-section">
            <summary>Advanced Settings</summary>
            <div class="form-row">
              <div class="form-group">
                <label>Temperature ({{ editing.temperature.toFixed(1) }})</label>
                <input v-model.number="editing.temperature" type="range" min="0" max="1" step="0.1" class="form-range" />
              </div>
              <div class="form-group">
                <label>Max Tokens</label>
                <input v-model.number="editing.max_tokens" type="number" class="form-input" min="256" max="32768" step="256" />
              </div>
              <div v-if="editing.model_tier === 'heavy'" class="form-group">
                <label>Reasoning Budget</label>
                <input v-model.number="editing.reasoning_budget" type="number" class="form-input" min="0" max="16384" step="512" />
              </div>
            </div>
          </details>

          <div class="edit-footer">
            <button class="btn-primary" @click="handleSave" :disabled="saving">
              {{ saving ? 'Saving...' : 'Save' }}
            </button>
            <button class="btn-secondary" @click="cancelEdit">Cancel</button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import { Plus, X, Trash2, Users } from 'lucide-vue-next'
import { useAgentConfigs, type AgentConfigDto } from '@/composables/useAgentConfigs'
import { useApiSources, type ApiSource } from '@/composables/useApiSources'
import { useSouls } from '@/composables/useSouls'

const {
  configs, loading, error,
  loadConfigs, createConfig, updateConfig, deleteConfig, resetDefaults,
} = useAgentConfigs()
const { sources: apiSources, loadSources } = useApiSources()
const { loadSouls, getSoulMarkdown } = useSouls()

const editing = ref<AgentConfigDto | null>(null)
const editingId = ref<string | null>(null)
const isNew = ref(false)
const saving = ref(false)
const soulMarkdown = ref('')

const professions = [
  { id: 'assistant', name: 'Assistant' },
  { id: 'advisor', name: 'Advisor' },
  { id: 'architect', name: 'Architect' },
  { id: 'planner', name: 'Planner' },
  { id: 'coder', name: 'Coder' },
  { id: 'tester', name: 'Tester' },
  { id: 'reviewer', name: 'Reviewer' },
  { id: 'documenter', name: 'Documenter' },
]

const tiers = [
  { value: 'light' as const, label: 'Light', bars: 1 },
  { value: 'mid' as const, label: 'Mid', bars: 2 },
  { value: 'heavy' as const, label: 'Heavy', bars: 3 },
]

const professionEmoji = (id: string) => {
  const map: Record<string, string> = {
    assistant: '🎯', advisor: '🔍', architect: '🏗️', planner: '📅',
    coder: '💻', tester: '🧪', reviewer: '📝', documenter: '📊',
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
  editing.value = { ...agent }
  editingId.value = agent.id
  isNew.value = false
  soulMarkdown.value = getSoulMarkdown(agent.soul_id)
}

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

async function handleResetDefaults() {
  if (!confirm('Reset to 8 default agents? Custom agents will be kept.')) return
  await resetDefaults()
}

onMounted(() => {
  loadConfigs()
  loadSources()
  loadSouls()
})
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
  margin-bottom: 1.5rem;
}

.agents-header h2 {
  font-size: 1.1rem;
  font-weight: 600;
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
  font-size: 0.85rem;
}

/* Grid */
.agents-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(260px, 1fr));
  gap: 1rem;
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

.card-avatar {
  width: 56px;
  height: 56px;
  border-radius: 12px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 1.8rem;
}

.card-avatar.assistant { background: hsl(25 80% 50% / 0.1); }
.card-avatar.advisor { background: hsl(280 60% 50% / 0.1); }
.card-avatar.architect { background: hsl(200 60% 50% / 0.1); }
.card-avatar.planner { background: hsl(140 60% 40% / 0.1); }
.card-avatar.coder { background: hsl(340 70% 50% / 0.1); }
.card-avatar.tester { background: hsl(60 70% 45% / 0.1); }
.card-avatar.reviewer { background: hsl(170 60% 40% / 0.1); }
.card-avatar.documenter { background: hsl(220 60% 50% / 0.1); }

.card-name {
  font-size: 0.95rem;
  font-weight: 600;
}

.card-badges {
  display: flex;
  gap: 0.35rem;
  flex-wrap: wrap;
}

.badge {
  font-size: 0.65rem;
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

.tier-badge.light { background: hsl(140 60% 40% / 0.12); color: hsl(140 60% 35%); }
.tier-badge.mid { background: hsl(210 60% 50% / 0.12); color: hsl(210 60% 45%); }
.tier-badge.heavy { background: hsl(280 50% 50% / 0.12); color: hsl(280 50% 45%); }

.default-badge {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-muted);
}

.card-soul-preview {
  font-size: 0.75rem;
  color: var(--af-muted);
  line-height: 1.4;
}

.card-actions {
  display: flex;
  gap: 0.3rem;
  margin-top: 0.3rem;
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
  font-size: 0.78rem;
  font-weight: 500;
}

.form-input, .form-select, .form-textarea {
  padding: 0.45rem 0.6rem;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  background: var(--af-bg);
  color: var(--af-fg);
  font-size: 0.82rem;
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
  font-size: 0.7rem;
  color: var(--af-muted);
}

.form-range {
  width: 100%;
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

.tier-option.active.light {
  border-color: hsl(140 60% 35%);
  background: hsl(140 60% 40% / 0.08);
}

.tier-option.active.mid {
  border-color: hsl(210 60% 45%);
  background: hsl(210 60% 50% / 0.08);
}

.tier-option.active.heavy {
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

.tier-option.active.light .tier-bar { background: hsl(140 60% 35%); }
.tier-option.active.mid .tier-bar { background: hsl(210 60% 45%); }
.tier-option.active.heavy .tier-bar { background: hsl(280 50% 45%); }

.tier-label {
  font-size: 0.7rem;
  font-weight: 500;
}

/* Advanced */
.advanced-section {
  border: 1px solid var(--af-border);
  border-radius: 6px;
  padding: 0.5rem 0.75rem;
}

.advanced-section summary {
  font-size: 0.78rem;
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
  font-size: 0.78rem;
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
</style>
