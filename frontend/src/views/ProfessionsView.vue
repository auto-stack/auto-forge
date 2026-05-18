<template>
  <div class="professions-view">
    <div class="professions-header">
      <h2>Professions</h2>
      <div class="header-actions">
        <button class="btn-secondary" @click="handleResetDefaults">Reset Defaults</button>
        <button class="btn-primary" @click="startCreate">
          <Plus :size="14" /> Add Profession
        </button>
      </div>
    </div>

    <div v-if="loading" class="professions-empty">Loading professions...</div>

    <div v-else-if="!professions.length" class="professions-empty">
      <Briefcase :size="48" />
      <p>No professions defined. Reset defaults to get started.</p>
    </div>

    <div v-else class="professions-grid">
      <div
        v-for="prof in professions"
        :key="prof.id"
        class="profession-card"
        :class="{ editing: editingId === prof.id }"
        @click="startEdit(prof)"
      >
        <div class="profession-header-row">
          <span class="profession-emoji">{{ professionEmoji(prof.id) }}</span>
          <div class="profession-name">{{ prof.name }}</div>
          <span class="phase-badge">{{ prof.phase }}</span>
        </div>
        <div class="profession-stats">
          <span class="stat">{{ prof.allowed_tools.length }} tools</span>
          <span class="stat">{{ prof.max_turns }} turns</span>
          <span class="stat">{{ (prof.token_budget / 1000).toFixed(0) }}k budget</span>
        </div>
        <div v-if="prof.base_skills?.length" class="profession-skills">
          <span v-for="sid in prof.base_skills" :key="sid" class="skill-chip">{{ skillName(sid) }}</span>
        </div>
        <div class="card-actions">
          <button class="btn-small" @click.stop="startEdit(prof)">Edit</button>
          <button class="btn-small btn-danger" @click.stop="handleDelete(prof.id)">
            <Trash2 :size="12" />
          </button>
        </div>
      </div>

      <div class="profession-card add-card" @click="startCreate">
        <Plus :size="24" class="add-icon" />
        <span>Add Profession</span>
      </div>
    </div>

    <!-- Edit Overlay -->
    <div v-if="editing" class="edit-overlay" @click.self="cancelEdit">
      <div class="edit-panel">
        <div class="edit-header">
          <h3>{{ isNew ? 'Create Profession' : editing.name }}</h3>
          <button class="btn-close" @click="cancelEdit"><X :size="16" /></button>
        </div>

        <div class="edit-form">
          <div class="form-row">
            <div class="form-group">
              <label>ID</label>
              <input v-model="editing.id" class="form-input" placeholder="unique-id" :disabled="!isNew" />
            </div>
            <div class="form-group">
              <label>Name</label>
              <input v-model="editing.name" class="form-input" placeholder="Profession name" />
            </div>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>Phase</label>
              <select v-model="editing.phase" class="form-select">
                <option v-for="ph in phases" :key="ph" :value="ph">{{ ph }}</option>
              </select>
            </div>
            <div class="form-group">
              <label>Max Turns</label>
              <input v-model.number="editing.max_turns" type="number" class="form-input" min="1" />
            </div>
            <div class="form-group">
              <label>Token Budget</label>
              <input v-model.number="editing.token_budget" type="number" class="form-input" min="0" step="1000" />
            </div>
          </div>

          <div class="form-group">
            <label>Owned Sections</label>
            <TagInput v-model="editing.owned_sections" placeholder="Add section and press Enter..." />
          </div>

          <div class="form-group">
            <label>Readable Sections</label>
            <TagInput v-model="editing.readable_sections" placeholder="Add section and press Enter..." />
          </div>

          <div class="form-group">
            <label>Allowed Tools</label>
            <TagInput v-model="editing.allowed_tools" placeholder="Add tool name and press Enter..." />
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>Handoff To</label>
              <TagInput v-model="editing.handoff_to" placeholder="profession ids..." />
            </div>
            <div class="form-group">
              <label>Approval Gates</label>
              <TagInput v-model="editing.approval_gates" placeholder="profession ids..." />
            </div>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>Dispatchable To</label>
              <TagInput v-model="editing.dispatchable_to" placeholder="profession ids..." />
            </div>
            <div class="form-group">
              <label>Base Skills</label>
              <div class="skills-selector">
                <label
                  v-for="skill in skills"
                  :key="skill.id"
                  class="skill-checkbox"
                  :class="{ checked: editing.base_skills?.includes(skill.id) }"
                >
                  <input
                    type="checkbox"
                    :checked="editing.base_skills?.includes(skill.id)"
                    @change="toggleBaseSkill(skill.id)"
                  />
                  <span class="skill-check-name">{{ skill.name }}</span>
                </label>
              </div>
            </div>
          </div>

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
import { ref, onMounted } from 'vue'
import { Plus, X, Trash2, Briefcase } from 'lucide-vue-next'
import { useProfessions, type ProfessionDto } from '@/composables/useProfessions'
import { useSkills } from '@/composables/useSkills'
import TagInput from '@/components/editors/TagInput.vue'

const {
  professions, loading, error,
  loadProfessions, createProfession, updateProfession, deleteProfession, resetDefaults,
} = useProfessions()
const { skills, loadSkills: loadSkillsList } = useSkills()

const editing = ref<ProfessionDto | null>(null)
const editingId = ref<string | null>(null)
const isNew = ref(false)
const saving = ref(false)

const phases = ['intake', 'discovery', 'goal_gate', 'design', 'planning', 'execution', 'verification', 'report', 'errand']

const professionEmoji = (id: string) => {
  const map: Record<string, string> = {
    assistant: '🎯', advisor: '🔍', architect: '🏗️', planner: '📅',
    coder: '💻', tester: '🧪', reviewer: '📝', documenter: '📊',
    gofer: '🔎',
  }
  return map[id] || '🤖'
}

function skillName(id: string): string {
  return skills.value.find(s => s.id === id)?.name || id
}

function startEdit(prof: ProfessionDto) {
  editing.value = { ...prof }
  editingId.value = prof.id
  isNew.value = false
}

function startCreate() {
  editing.value = {
    id: '',
    name: '',
    phase: 'execution',
    owned_sections: [],
    readable_sections: [],
    allowed_tools: [],
    handoff_to: [],
    dispatchable_to: [],
    approval_gates: [],
    max_turns: 10,
    token_budget: 8000,
    base_skills: [],
  }
  editingId.value = null
  isNew.value = true
}

function cancelEdit() {
  editing.value = null
  editingId.value = null
}

function toggleBaseSkill(skillId: string) {
  if (!editing.value) return
  const current = editing.value.base_skills || []
  if (current.includes(skillId)) {
    editing.value.base_skills = current.filter(id => id !== skillId)
  } else {
    editing.value.base_skills = [...current, skillId]
  }
}

async function handleSave() {
  if (!editing.value) return
  if (!editing.value.id.trim() || !editing.value.name.trim()) {
    alert('ID and Name are required')
    return
  }
  saving.value = true
  if (isNew.value) {
    const ok = await createProfession(editing.value)
    if (ok) cancelEdit()
  } else {
    const ok = await updateProfession(editing.value.id, editing.value)
    if (ok) cancelEdit()
  }
  saving.value = false
}

async function handleDelete(id: string) {
  if (!confirm('Delete this profession?')) return
  await deleteProfession(id)
}

async function handleResetDefaults() {
  if (!confirm('Reset to 9 default professions? Custom professions will be lost.')) return
  await resetDefaults()
}

onMounted(() => {
  loadProfessions()
  loadSkillsList()
})
</script>

<style scoped>
.professions-view {
  height: 100%;
  overflow-y: auto;
  padding: 1.5rem;
}

.professions-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1.5rem;
}

.professions-header h2 {
  font-size: 1.3rem;
  font-weight: 600;
}

.header-actions {
  display: flex;
  gap: 0.5rem;
}

.professions-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 50%;
  color: var(--af-muted);
  gap: 0.75rem;
  font-size: 0.93rem;
}

.professions-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1rem;
  max-width: 1200px;
  margin: 0 auto;
  align-items: stretch;
}

.profession-card {
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

.profession-card:hover {
  border-color: hsl(var(--primary) / 0.3);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

.profession-card.editing {
  border-color: var(--af-primary);
  box-shadow: 0 0 0 2px hsl(var(--primary) / 0.15);
}

.profession-card.add-card {
  align-items: center;
  justify-content: center;
  color: var(--af-muted);
  gap: 0.5rem;
  min-height: 180px;
}

.profession-card.add-card:hover {
  color: var(--af-primary);
  border-color: hsl(var(--primary) / 0.3);
}

.add-icon {
  opacity: 0.5;
}

.profession-card.add-card:hover .add-icon {
  opacity: 1;
}

.profession-header-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.profession-emoji {
  font-size: 1.2rem;
}

.profession-name {
  font-size: 1.03rem;
  font-weight: 600;
  flex: 1;
}

.phase-badge {
  font-size: 0.7rem;
  font-weight: 600;
  padding: 0.1rem 0.4rem;
  border-radius: 4px;
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.profession-stats {
  display: flex;
  gap: 0.5rem;
  flex-wrap: wrap;
}

.stat {
  font-size: 0.78rem;
  color: var(--af-muted);
}

.profession-skills {
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
  width: 620px;
  max-height: 85vh;
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

.skills-selector {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  max-height: 120px;
  overflow-y: auto;
  border: 1px solid var(--af-border);
  border-radius: 6px;
  padding: 0.4rem;
}

.skill-checkbox {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.25rem 0.4rem;
  border-radius: 4px;
  cursor: pointer;
  transition: background 0.1s;
  font-size: 0.85rem;
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
</style>
