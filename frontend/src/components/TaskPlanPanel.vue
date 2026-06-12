<template>
  <div class="task-plan-panel">
    <div v-if="error" class="error-banner">{{ error }}</div>

    <div class="tp-body">
      <!-- Left: plan list -->
      <div class="tp-sidebar">
        <div class="sidebar-header">
          <div class="panel-title">Task Plans</div>
          <button class="btn-icon" title="Refresh" @click="refresh">
            <RefreshCw :size="14" />
          </button>
        </div>
        <div class="tp-list">
          <div v-if="plans.length === 0" class="empty-state">No TaskPlans yet</div>
          <div
            v-for="plan in plans"
            :key="plan.id"
            class="tp-card"
            :class="{ active: currentPlan?.id === plan.id }"
            @click="selectPlan(plan.id)"
          >
            <div class="tp-card-header">
              <span class="tp-id">{{ plan.id }}</span>
              <span class="tp-source">{{ plan.source }}</span>
            </div>
            <div class="tp-card-meta">
              {{ plan.phase_count }} phases · {{ plan.run_count }} runs
            </div>
          </div>
        </div>
      </div>

      <!-- Right: detail -->
      <div class="tp-detail">
        <div v-if="!currentPlan" class="empty-state">Select a TaskPlan</div>
        <template v-else>
          <div class="panel-header">
            <div>
              <div class="panel-header-title">{{ currentPlan.title || currentPlan.id }}</div>
              <div v-if="currentPlan.description" class="panel-header-subtitle">{{ currentPlan.description }}</div>
            </div>
            <div class="tp-actions">
              <input
                v-model="initialInput"
                class="tp-input"
                placeholder="Initial task"
                @keydown.enter="startRun"
              />
              <button class="btn-primary" :disabled="!initialInput || loading" @click="startRun">
                <Play :size="14" /> Start
              </button>
            </div>
          </div>

          <div class="tp-phases">
            <div v-for="phase in currentPlan.phases" :key="phase.name" class="tp-phase">
              <div class="tp-phase-header">
                <span class="tp-phase-name">{{ phase.name }}</span>
                <span class="tp-phase-mode">{{ phase.mode }}</span>
                <span v-if="phase.depends_on.length" class="tp-phase-deps">
                  depends on {{ phase.depends_on.join(', ') }}
                </span>
              </div>
              <div class="tp-runs">
                <div v-for="run in phase.runs" :key="run.name" class="tp-run">
                  <span class="tp-run-name">{{ run.name }}</span>
                  <span class="tp-run-flow">{{ run.flow_id }}</span>
                </div>
              </div>
            </div>
          </div>
        </template>
      </div>
    </div>

    <!-- Bottom: registration -->
    <div class="tp-register">
      <div class="panel-title">Register TaskPlan</div>
      <textarea v-model="atomText" class="tp-atom" placeholder="Paste Atom TaskPlan source here" />
      <div class="tp-register-actions">
        <button class="btn-secondary" :disabled="!atomText || loading" @click="onValidate">Validate</button>
        <button class="btn-primary" :disabled="!atomText || loading" @click="onRegister">Register</button>
      </div>
      <div v-if="validationResult" class="tp-validation" :class="{ valid: validationResult.valid }">
        {{ validationResult.valid ? 'Valid TaskPlan' : validationResult.error }}
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'
import { Play, RefreshCw } from 'lucide-vue-next'
import { useTaskPlan } from '@/composables/useTaskPlan'
import { useProject } from '@/composables/useProject'

const { projectPath } = useProject()
const {
  plans, currentPlan, runs, loading, error,
  loadTaskPlans, getTaskPlan, startTaskPlanRun, loadTaskPlanRuns,
  registerTaskPlan, validateTaskPlan, subscribeToTaskPlan,
} = useTaskPlan()

const initialInput = ref('')
const atomText = ref('')
const validationResult = ref<{ valid: boolean; error?: string } | null>(null)
const unsubscribers: (() => void)[] = []

onMounted(() => {
  refresh()
})

watch(projectPath, () => {
  refresh()
})

async function refresh() {
  await loadTaskPlans()
  await loadTaskPlanRuns()
}

async function selectPlan(id: string) {
  await getTaskPlan(id)
  initialInput.value = ''
}

async function startRun() {
  if (!currentPlan.value || !initialInput.value) return
  const resp = await startTaskPlanRun(currentPlan.value.id, initialInput.value)
  if (resp) {
    const unsub = subscribeToTaskPlan(resp.instance_id, (ev) => {
      if (ev.event_type === 'task_plan_completed' || ev.event_type === 'task_plan_failed') {
        loadTaskPlanRuns()
      }
    })
    unsubscribers.push(unsub)
    initialInput.value = ''
  }
}

async function onValidate() {
  validationResult.value = await validateTaskPlan(atomText.value)
}

async function onRegister() {
  const plan = await registerTaskPlan(atomText.value)
  if (plan) {
    atomText.value = ''
    validationResult.value = null
    currentPlan.value = plan
  }
}
</script>

<style scoped>
.task-plan-panel {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
}

.error-banner {
  padding: 0.5rem 1rem;
  background: hsl(0 70% 50% / 0.08);
  color: hsl(0 70% 45%);
  border-bottom: 1px solid var(--af-border);
}

.tp-body {
  flex: 1;
  display: grid;
  grid-template-columns: 240px 1fr;
  gap: 1px;
  background: var(--af-border);
  overflow: hidden;
}

.tp-sidebar, .tp-detail {
  background: var(--af-bg);
  display: flex;
  flex-direction: column;
  overflow: hidden;
}

.sidebar-header, .panel-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  height: 48px;
  padding: 0.75rem 1rem;
  border-bottom: 1px solid var(--af-border);
  flex-shrink: 0;
}

.tp-list {
  flex: 1;
  overflow-y: auto;
  padding: 0.75rem;
}

.tp-card {
  padding: 0.6rem;
  border-radius: 6px;
  border: 1px solid var(--af-border);
  margin-bottom: 0.5rem;
  cursor: pointer;
  transition: all 0.15s;
}

.tp-card:hover, .tp-card.active {
  border-color: hsl(var(--primary) / 0.3);
  background: hsl(var(--primary) / 0.03);
}

.tp-card-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.25rem;
}

.tp-id {
  font-size: 0.83rem;
  font-weight: 500;
  font-family: 'JetBrains Mono', monospace;
}

.tp-source {
  font-size: 0.72rem;
  text-transform: uppercase;
  color: var(--af-muted);
}

.tp-card-meta {
  font-size: 0.78rem;
  color: var(--af-muted);
}

.panel-title {
  font-size: 0.78rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--af-muted);
}

.panel-header-title {
  font-size: 0.95rem;
  font-weight: 500;
}

.panel-header-subtitle {
  font-size: 0.78rem;
  color: var(--af-muted);
}

.tp-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.tp-input {
  min-width: 240px;
  padding: 0.35rem 0.6rem;
  border-radius: 4px;
  border: 1px solid var(--af-border);
  background: var(--af-bg);
  color: var(--af-fg);
  font-size: 0.85rem;
}

.tp-phases {
  flex: 1;
  overflow-y: auto;
  padding: 0.75rem;
}

.tp-phase {
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.75rem;
  margin-bottom: 0.75rem;
}

.tp-phase-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  margin-bottom: 0.5rem;
}

.tp-phase-name {
  font-weight: 600;
  font-size: 0.88rem;
}

.tp-phase-mode, .tp-phase-deps {
  font-size: 0.75rem;
  color: var(--af-muted);
  text-transform: uppercase;
}

.tp-runs {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.tp-run {
  display: flex;
  justify-content: space-between;
  padding: 0.4rem 0.5rem;
  background: hsl(var(--muted-foreground) / 0.04);
  border-radius: 4px;
  font-size: 0.83rem;
}

.tp-run-flow {
  color: var(--af-muted);
  font-size: 0.78rem;
}

.tp-register {
  border-top: 1px solid var(--af-border);
  padding: 0.75rem 1rem;
  background: var(--af-bg);
  flex-shrink: 0;
}

.tp-atom {
  width: 100%;
  height: 120px;
  margin-top: 0.5rem;
  padding: 0.5rem;
  border-radius: 4px;
  border: 1px solid var(--af-border);
  background: var(--af-bg);
  color: var(--af-fg);
  font-family: 'JetBrains Mono', monospace;
  font-size: 0.8rem;
  resize: vertical;
}

.tp-register-actions {
  display: flex;
  gap: 0.5rem;
  margin-top: 0.5rem;
}

.tp-validation {
  margin-top: 0.5rem;
  font-size: 0.83rem;
  color: hsl(0 70% 45%);
}

.tp-validation.valid {
  color: hsl(142 70% 35%);
}

.empty-state {
  font-size: 0.88rem;
  color: var(--af-muted);
  text-align: center;
  padding: 2rem 0;
}

.btn-primary, .btn-secondary, .btn-icon {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.4rem 0.7rem;
  border-radius: 6px;
  border: none;
  font-size: 0.83rem;
  cursor: pointer;
  transition: all 0.15s;
}

.btn-primary {
  background: var(--af-primary);
  color: white;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary, .btn-icon {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-fg);
}

.btn-icon {
  padding: 0.25rem;
  background: transparent;
  color: var(--af-muted);
}
</style>
