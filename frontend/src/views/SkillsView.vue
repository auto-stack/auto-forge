<template>
  <div class="skills-view">
    <div class="skills-header">
      <h2>Skills</h2>
      <div class="header-actions">
        <button class="btn-secondary" @click="handleResetDefaults">Reset Defaults</button>
        <button class="btn-primary" @click="startCreate">
          <Plus :size="14" /> Add Skill
        </button>
      </div>
    </div>

    <div v-if="loading" class="skills-empty">Loading skills...</div>

    <div v-else-if="!skills.length" class="skills-empty">
      <Wrench :size="48" />
      <p>No skills defined yet. Add one or reset defaults to get started.</p>
    </div>

    <div v-else class="skills-grid">
      <div
        v-for="skill in skills"
        :key="skill.id"
        class="skill-card"
        :class="{ editing: editingId === skill.id }"
        @click="startEdit(skill)"
      >
        <div class="skill-header-row">
          <span class="skill-icon">🛠️</span>
          <div class="skill-name">{{ skill.name }}</div>
        </div>
        <div class="skill-description">{{ skill.description }}</div>
        <div class="skill-badges">
          <span class="badge tool-badge">{{ skill.granted_tools.length }} tools</span>
          <span v-if="skill.extra_turns > 0" class="badge extra-badge">+{{ skill.extra_turns }} turns</span>
        </div>
        <div class="skill-tools" v-if="skill.granted_tools.length">
          <span v-for="tool in skill.granted_tools" :key="tool" class="tool-chip">{{ tool }}</span>
        </div>
        <div class="card-actions">
          <button class="btn-small" @click.stop="startEdit(skill)">Edit</button>
          <button class="btn-small btn-danger" @click.stop="handleDelete(skill.id)">
            <Trash2 :size="12" />
          </button>
        </div>
      </div>

      <!-- Add card -->
      <div class="skill-card add-card" @click="startCreate">
        <Plus :size="24" class="add-icon" />
        <span>Add Skill</span>
      </div>
    </div>

    <!-- Edit Overlay -->
    <div v-if="editing" class="edit-overlay" @click.self="cancelEdit">
      <div class="edit-panel">
        <div class="edit-header">
          <h3>{{ isNew ? 'Create Skill' : editing.name }}</h3>
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
              <input v-model="editing.name" class="form-input" placeholder="Skill name" />
            </div>
          </div>

          <div class="form-group">
            <label>Description</label>
            <textarea v-model="editing.description" class="form-textarea" rows="2" placeholder="Short description..." />
          </div>

          <div class="form-group">
            <label>Granted Tools</label>
            <TagInput v-model="editing.granted_tools" placeholder="Add tool name and press Enter..." />
            <div class="form-hint">Tools this skill unlocks for equipped agents.</div>
          </div>

          <div class="form-group">
            <label>Prompt Fragment</label>
            <textarea v-model="editing.prompt_fragment" class="form-textarea" rows="5" placeholder="Instructions injected into the system prompt..." />
            <div class="form-hint">Markdown instructions added to the agent's system prompt.</div>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>Extra Turns</label>
              <input v-model.number="editing.extra_turns" type="number" class="form-input" min="0" />
            </div>
            <div class="form-group">
              <label>Extra Token Budget</label>
              <input v-model.number="editing.extra_token_budget" type="number" class="form-input" min="0" step="1000" />
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
import { Plus, X, Trash2, Wrench } from 'lucide-vue-next'
import { useSkills, type SkillDto } from '@/composables/useSkills'
import TagInput from '@/components/editors/TagInput.vue'

const {
  skills, loading, error,
  loadSkills, createSkill, updateSkill, deleteSkill, resetDefaults,
} = useSkills()

const editing = ref<SkillDto | null>(null)
const editingId = ref<string | null>(null)
const isNew = ref(false)
const saving = ref(false)

function startEdit(skill: SkillDto) {
  editing.value = { ...skill }
  editingId.value = skill.id
  isNew.value = false
}

function startCreate() {
  editing.value = {
    id: '',
    name: '',
    description: '',
    granted_tools: [],
    prompt_fragment: '',
    extra_turns: 0,
    extra_token_budget: 0,
  }
  editingId.value = null
  isNew.value = true
}

function cancelEdit() {
  editing.value = null
  editingId.value = null
}

async function handleSave() {
  if (!editing.value) return
  if (!editing.value.id.trim() || !editing.value.name.trim()) {
    alert('ID and Name are required')
    return
  }
  saving.value = true
  if (isNew.value) {
    const ok = await createSkill(editing.value)
    if (ok) cancelEdit()
  } else {
    const ok = await updateSkill(editing.value.id, editing.value)
    if (ok) cancelEdit()
  }
  saving.value = false
}

async function handleDelete(id: string) {
  if (!confirm('Delete this skill?')) return
  await deleteSkill(id)
}

async function handleResetDefaults() {
  if (!confirm('Reset to default skills? Custom skills will be lost.')) return
  await resetDefaults()
}

onMounted(() => {
  loadSkills()
})
</script>

<style scoped>
.skills-view {
  height: 100%;
  overflow-y: auto;
  padding: 1.5rem;
}

.skills-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  height: 48px;
  flex-shrink: 0;
  border-bottom: 1px solid var(--af-border);
  gap: 1rem;
}

.skills-header h2 {
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

.skills-empty {
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
.skills-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1rem;
  max-width: 1200px;
  margin: 0 auto;
  align-items: stretch;
}

.skill-card {
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

.skill-card:hover {
  border-color: hsl(var(--primary) / 0.3);
  box-shadow: 0 2px 8px rgba(0, 0, 0, 0.06);
}

.skill-card.editing {
  border-color: var(--af-primary);
  box-shadow: 0 0 0 2px hsl(var(--primary) / 0.15);
}

.skill-card.add-card {
  align-items: center;
  justify-content: center;
  color: var(--af-muted);
  gap: 0.5rem;
  min-height: 180px;
}

.skill-card.add-card:hover {
  color: var(--af-primary);
  border-color: hsl(var(--primary) / 0.3);
}

.add-icon {
  opacity: 0.5;
}

.skill-card.add-card:hover .add-icon {
  opacity: 1;
}

.skill-header-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.skill-icon {
  font-size: 1.2rem;
}

.skill-name {
  font-size: 1.03rem;
  font-weight: 600;
}

.skill-description {
  font-size: 0.83rem;
  color: var(--af-muted);
  line-height: 1.4;
  flex: 1;
}

.skill-badges {
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

.tool-badge {
  background: hsl(var(--primary) / 0.1);
  color: var(--af-primary);
}

.extra-badge {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-muted);
}

.skill-tools {
  display: flex;
  flex-wrap: wrap;
  gap: 0.3rem;
}

.tool-chip {
  font-size: 0.75rem;
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
  background: hsl(var(--muted-foreground) / 0.06);
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
</style>
