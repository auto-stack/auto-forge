<template>
  <div class="agents-view" data-testid="relay-view">
    <div class="agents-header">
      <h2>Agents Relay</h2>
      <div class="agents-actions">
        <button class="btn-primary" @click="showStartModal = true" :disabled="loading">
          <Play :size="14" />
          Start Run
        </button>
        <button class="btn-secondary" @click="refresh">
          <RefreshCw :size="14" />
        </button>
      </div>
    </div>

    <div v-if="error" class="error-banner">{{ error }}</div>

    <div class="agents-body">
      <!-- Left: Runs list -->
      <div class="runs-sidebar" data-testid="relay-run-list">
        <div class="panel-title">Runs</div>
        <div v-if="runs.length === 0" class="empty-state">No runs yet</div>
        <div
          v-for="run in runs"
          :key="run.run_id"
          class="run-card"
          :class="{ active: currentRun?.run_id === run.run_id }"
          :data-testid="`run-card-${run.run_id}`"
          :title="run.task ? run.task.slice(0, 50) : ''"
          @click="selectRun(run.run_id)"
        >
          <div class="run-card-header">
            <span class="run-id" :title="run.run_id">{{ run.title || run.run_id }}</span>
            <StatusBadge :status="run.status" />
          </div>
          <div class="run-card-meta">
            <span>{{ run.current_profession ?? '—' }}</span>
            <span>{{ formatTokens(run.cumulative_tokens) }}</span>
          </div>
          <div class="run-progress-bar">
            <div
              class="run-progress-fill"
              :style="{ width: runProgressPercent(run) + '%' }"
            />
          </div>
          <button
            class="btn-icon btn-delete"
            title="Delete run"
            @click.stop="onDeleteRun(run.run_id)"
          >
            <Trash2 :size="12" />
          </button>
        </div>
      </div>

      <!-- Center: Pipeline visualization -->
      <div class="pipeline-panel" data-testid="pipeline-panel">
        <div v-if="!currentRun" class="empty-state">
          Select a run or start a new one
        </div>

        <template v-else>
          <!-- Run header -->
          <div class="run-header">
            <div class="run-title">{{ currentRun.title || currentRun.run_id }}</div>
            <div class="run-stats">
              <span class="stat-badge">
                <Coins :size="12" />
                {{ formatTokens(currentRun.cumulative_tokens) }}
              </span>
              <span class="stat-badge">
                <Zap :size="12" />
                {{ Math.round(currentRun.savings_ratio * 100) }}% saved
              </span>
            </div>
          </div>

          <!-- Budget bar -->
          <div class="budget-bar-container">
            <div class="budget-label">
              <span>Budget</span>
              <span>{{ formatTokens(currentRun.budget_limit - currentRun.budget_remaining) }} / {{ formatTokens(currentRun.budget_limit) }}</span>
            </div>
            <div class="budget-bar">
              <div
                class="budget-fill"
                :class="{ warning: budgetUsedPercent > 70, danger: budgetUsedPercent > 90 }"
                :style="{ width: budgetUsedPercent + '%' }"
              />
            </div>
          </div>

          <!-- Pipeline steps -->
          <div class="pipeline-flow" data-testid="pipeline-flow">
            <template v-for="(step, idx) in currentRun.steps" :key="step.id">
              <div
                class="pipeline-step"
                :class="[step.status, { expanded: expandedStepId === step.id }]"
                :title="`${step.profession_id} (${step.gate})`"
                data-testid="pipeline-step"
                :aria-label="`${step.profession_id} step, status ${step.status}${step.gate === 'human' ? ', human gate required' : ''}`"
                role="button"
                tabindex="0"
                @click="toggleStep(step.id)"
                @keydown.enter="toggleStep(step.id)"
              >
                <AgentAvatar :profession-id="step.profession_id" size="sm" aria-hidden="true" />
                <div class="step-name">{{ step.profession_id }}</div>
                <div v-if="step.gate === 'human'" class="step-gate" aria-hidden="true">🔒</div>
                <div v-if="step.status === 'running'" class="step-pulse" aria-hidden="true" />
                <div v-if="stepIteration(step.id) > 1" class="step-retry" aria-label="Retry iteration {{ stepIteration(step.id) }}">
                  ×{{ stepIteration(step.id) }}
                </div>
              </div>

              <!-- Expanded node card -->
              <div
                v-if="expandedStepId === step.id"
                class="expanded-step-card"
              >
                <div class="expanded-header">
                  <AgentAvatar :profession-id="step.profession_id" size="md" />
                  <span class="expanded-name">{{ step.profession_id }}</span>
                  <StatusBadge :status="step.status" size="sm" />
                </div>
                <div class="expanded-metrics">
                  <div class="metric">
                    <span class="metric-label">Gate</span>
                    <span class="metric-value">{{ step.gate }}</span>
                  </div>
                  <div class="metric">
                    <span class="metric-label">Iterations</span>
                    <span class="metric-value">{{ stepIteration(step.id) }}</span>
                  </div>
                  <div class="metric">
                    <span class="metric-label">Tokens</span>
                    <span class="metric-value">{{ formatTokens(stepTokens(step.id)) }}</span>
                  </div>
                </div>
                <div class="expanded-actions">
                  <span class="expanded-hint">Click step to collapse</span>
                </div>
              </div>

              <div v-if="idx < currentRun.steps.length - 1" class="step-connector">
                <ChevronRight :size="14" />
              </div>
            </template>
          </div>

          <!-- Session Log -->
          <div class="session-log-panel">
            <div class="panel-title">Session Log</div>
            <div v-if="sessionLog.length === 0" class="empty-state">
              Start a run to see agent activity
            </div>
            <div v-else ref="sessionLogRef" class="session-log-list">
              <div
                v-for="entry in sessionLog"
                :key="entry.id"
                class="session-entry"
                :class="`type-${entry.type}`"
              >
                <div class="session-entry-header">
                  <AgentAvatar :profession-id="entry.profession_id" size="xs" />
                  <span class="session-profession">{{ entry.profession_id }}</span>
                  <span class="session-time">{{ entry.time }}</span>
                </div>

                <!-- Text content -->
                <div v-if="entry.type === 'text'" class="session-text">
                  <pre>{{ entry.content }}</pre>
                </div>

                <!-- Tool call -->
                <div v-else-if="entry.type === 'tool_call'" class="session-tool">
                  <div class="tool-header">
                    <Wrench :size="12" />
                    <span>{{ entry.tool_name }}</span>
                  </div>
                  <details class="tool-details">
                    <summary>Arguments</summary>
                    <pre>{{ JSON.stringify(entry.arguments, null, 2) }}</pre>
                  </details>
                </div>

                <!-- Tool result -->
                <div v-else-if="entry.type === 'tool_result'" class="session-tool result">
                  <div class="tool-header">
                    <CheckCircle :size="12" />
                    <span>Result</span>
                  </div>
                  <details class="tool-details">
                    <summary>Output</summary>
                    <pre>{{ entry.content }}</pre>
                  </details>
                </div>

                <!-- Complete -->
                <div v-else-if="entry.type === 'complete'" class="session-complete">
                  <CheckCircle :size="12" />
                  <span>Turn completed</span>
                </div>

                <!-- Error -->
                <div v-else-if="entry.type === 'error'" class="session-error">
                  <AlertCircle :size="12" />
                  <span>{{ entry.content }}</span>
                </div>

                <!-- Budget warning -->
                <div v-else-if="entry.type === 'budget_warning'" class="session-warning">
                  <AlertTriangle :size="12" />
                  <span>{{ entry.content }}</span>
                </div>

                <!-- Budget exceeded -->
                <div v-else-if="entry.type === 'budget_exceeded'" class="session-error">
                  <AlertCircle :size="12" />
                  <span>{{ entry.content }}</span>
                </div>

                <!-- Step started -->
                <div v-else-if="entry.type === 'step_started'" class="session-step">
                  <Play :size="12" />
                  <span>{{ entry.content }}</span>
                </div>

                <!-- Step completed -->
                <div v-else-if="entry.type === 'step_completed'" class="session-step completed">
                  <CheckCircle :size="12" />
                  <span>{{ entry.content }}</span>
                </div>

                <!-- Gate waiting -->
                <div v-else-if="entry.type === 'gate_waiting'" class="session-warning">
                  <AlertTriangle :size="12" />
                  <span>{{ entry.content }}</span>
                </div>

                <!-- Run completed -->
                <div v-else-if="entry.type === 'run_completed'" class="session-step completed">
                  <CheckCircle :size="12" />
                  <span>{{ entry.content }}</span>
                </div>

                <!-- Run failed -->
                <div v-else-if="entry.type === 'run_failed'" class="session-error">
                  <AlertCircle :size="12" />
                  <span>{{ entry.content }}</span>
                </div>
              </div>
            </div>
          </div>

          <!-- Cost Breakdown -->
          <div class="cost-panel">
            <div class="panel-title">Cost Breakdown</div>
            <div v-if="!currentRun" class="empty-state">—</div>
            <div v-else class="cost-bars">
              <div class="cost-bar-row">
                <span class="cost-label">Total</span>
                <div class="cost-bar-bg">
                  <div
                    class="cost-bar-fill"
                    :style="{ width: '100%' }"
                  />
                </div>
                <span class="cost-value">{{ formatTokens(currentRun.cumulative_tokens) }}</span>
              </div>
              <div
                v-for="(tokens, prof) in professionTokens"
                :key="prof"
                class="cost-bar-row"
              >
                <span class="cost-label">{{ prof }}</span>
                <div class="cost-bar-bg">
                  <div
                    class="cost-bar-fill profession"
                    :style="{ width: Math.min(100, (tokens / Math.max(currentRun.cumulative_tokens, 1)) * 100) + '%' }"
                  />
                </div>
                <span class="cost-value">{{ formatTokens(tokens) }}</span>
              </div>
              <div v-if="Object.keys(professionTokens).length === 0" class="empty-state">
                Per-profession data will appear as steps run
              </div>
            </div>
          </div>

          <!-- Gate approval panel -->
          <GatePanel
            v-if="showGatePanel && currentRun.waiting_for_gate"
            :run-id="currentRun.run_id"
            :gate="currentGate!"
            :profession-id="currentRun.waiting_for_gate.profession_id"
            @approve="onApprove"
            @reject="onReject"
            @review-in-specs="onReviewInSpecs"
          />

          <!-- Step history -->
          <div class="history-panel">
            <div class="panel-title">Step History</div>
            <div v-if="currentRun.step_history.length === 0" class="empty-state">No steps completed yet</div>
            <div
              v-for="record in currentRun.step_history"
              :key="record.step_id + record.started_at"
              class="history-row"
            >
              <span class="history-profession">{{ record.profession_id }}</span>
              <span class="history-time">{{ formatTime(record.completed_at) }}</span>
            </div>
          </div>
        </template>
      </div>

    </div>

    <!-- Start Run Modal -->
    <div v-if="showStartModal" class="modal-overlay" @click.self="showStartModal = false">
      <div class="modal-content">
        <h3>Start New Run</h3>
        <div class="form-group">
          <label>Flow ID</label>
          <input v-model="newFlowId" placeholder="e.g. feature-auth" />
        </div>
        <div class="form-group">
          <label>Task / Initial Prompt</label>
          <textarea v-model="newTask" placeholder="Describe what the relay should build or investigate..." rows="3" />
        </div>
        <div class="form-group">
          <label>Steps</label>
          <div class="steps-builder">
            <div v-for="(step, i) in newSteps" :key="i" class="step-row">
              <input v-model="step.id" placeholder="step-id" class="step-input" />
              <select v-model="step.profession_id" class="step-select">
                <option v-for="p in professions" :key="p.id" :value="p.id">{{ p.name }}</option>
              </select>
              <select v-model="step.gate" class="step-select">
                <option value="auto">Auto</option>
                <option value="human">Human</option>
              </select>
              <button class="btn-icon" @click="removeStep(i)">
                <Trash2 :size="14" />
              </button>
            </div>
            <button class="btn-add" @click="addStep">
              <Plus :size="14" />
              Add Step
            </button>
          </div>
        </div>
        <div class="modal-actions">
          <button class="btn-secondary" @click="showStartModal = false">Cancel</button>
          <button class="btn-primary" :disabled="loading" @click="onStartRun">
            <Play :size="14" />
            Start
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import {
  Play, RefreshCw, Coins, Zap, ChevronRight,
  Trash2, Plus, Wrench, CheckCircle, AlertCircle, AlertTriangle,
} from 'lucide-vue-next'
import { useRelay } from '@/composables/useRelay'
import { useGateInbox } from '@/composables/useGateInbox'
import { useForgeMode } from '@/composables/useForgeMode'
import StatusBadge from '@/components/StatusBadge.vue'
import GatePanel from '@/components/GatePanel.vue'
import AgentAvatar from '@/components/AgentAvatar.vue'

const {
  runs, currentRun, professions, souls, loading, error,
  hasActiveGate, budgetUsedPercent, liveLog, professionTokens, sessionLog,
  loadProfessions, loadSouls, loadRuns, loadRun, startRun,
  resolveGate, subscribeToRun, deleteRun,
} = useRelay()

const gateInbox = useGateInbox()
const { shouldPauseGate } = useForgeMode()

const showStartModal = ref(false)
const expandedStepId = ref<string | null>(null)
const sessionLogRef = ref<HTMLElement | null>(null)

// Auto-scroll session log to bottom when new entries arrive
import { watch, nextTick } from 'vue'
watch(() => sessionLog.value.length, async () => {
  await nextTick()
  if (sessionLogRef.value) {
    sessionLogRef.value.scrollTop = sessionLogRef.value.scrollHeight
  }
})

function toggleStep(stepId: string) {
  expandedStepId.value = expandedStepId.value === stepId ? null : stepId
}

function stepIteration(stepId: string): number {
  if (!currentRun.value) return 0
  return currentRun.value.step_history.filter((h) => h.step_id === stepId).length
}

function stepTokens(stepId: string): number {
  // Derive from profession tokens if available, else estimate from history
  const step = currentRun.value?.steps.find((s) => s.id === stepId)
  if (!step) return 0
  return professionTokens.value[step.profession_id] || 0
}

const showGatePanel = computed(() => {
  if (!hasActiveGate.value || !currentRun.value?.waiting_for_gate) return false
  // In GSD mode, only show gates that are goal-level
  return shouldPauseGate(currentRun.value.waiting_for_gate.profession_id)
})

const currentGate = computed(() => {
  if (!currentRun.value?.waiting_for_gate) return null
  return {
    gateId: `${currentRun.value.run_id}-${currentRun.value.waiting_for_gate.step_id}`,
    runId: currentRun.value.run_id,
    profession: currentRun.value.waiting_for_gate.profession_id,
    title: `${currentRun.value.waiting_for_gate.profession_id} needs approval`,
    since: currentRun.value.waiting_for_gate.since,
    status: 'pending' as const,
  }
})
const newFlowId = ref('demo-flow')
const newTask = ref('')
const newSteps = ref<{ id: string; profession_id: string; gate: string }[]>([
  { id: 'intake', profession_id: 'assistant', gate: 'auto' },
  { id: 'discover', profession_id: 'advisor', gate: 'human' },
  { id: 'design', profession_id: 'architect', gate: 'auto' },
  { id: 'plan', profession_id: 'planner', gate: 'auto' },
  { id: 'draft-tests', profession_id: 'tester', gate: 'auto' },
  { id: 'code', profession_id: 'coder', gate: 'auto' },
  { id: 'run-tests', profession_id: 'tester', gate: 'auto' },
  { id: 'review', profession_id: 'reviewer', gate: 'auto' },
  { id: 'report', profession_id: 'documenter', gate: 'auto' },
])

let unsubscribe: (() => void) | null = null

onMounted(async () => {
  await loadProfessions()
  await loadSouls()
  await loadRuns()
})

onUnmounted(() => {
  if (unsubscribe) unsubscribe()
})

function selectRun(runId: string) {
  if (unsubscribe) unsubscribe()
  sessionLog.value = []
  loadRun(runId)
  unsubscribe = subscribeToRun(runId)
}

async function refresh() {
  await loadRuns()
  if (currentRun.value) {
    await loadRun(currentRun.value.run_id)
  }
}

function addStep() {
  newSteps.value.push({ id: '', profession_id: 'coder', gate: 'auto' })
}

function removeStep(i: number) {
  newSteps.value.splice(i, 1)
}

async function onStartRun() {
  const runId = await startRun({
    flow_id: newFlowId.value,
    steps: newSteps.value,
    task: newTask.value,
  })
  if (runId) {
    showStartModal.value = false
    newTask.value = ''
    selectRun(runId)
  }
}

async function onDeleteRun(runId: string) {
  if (confirm('Delete this run?')) {
    await deleteRun(runId)
  }
}

async function onApprove(runId: string) {
  await resolveGate(runId, 'approve')
}

async function onReject(runId: string) {
  await resolveGate(runId, 'reject', 'Needs revision')
}

function onReviewInSpecs(sectionId: string) {
  alert(`Navigate to specs section: ${sectionId}`)
}

function formatTokens(n: number): string {
  if (n >= 1000) return `${(n / 1000).toFixed(1)}k`
  return `${n}`
}

function formatTime(ts: number): string {
  return new Date(ts * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function runProgressPercent(run: { current_step: number; total_steps: number }): number {
  if (run.total_steps === 0) return 0
  return Math.round((run.current_step / run.total_steps) * 100)
}

function professionIcon(id: string): string {
  const map: Record<string, string> = {
    assistant: '📥', advisor: '💡', planner: '📝', architect: '🏗️',
    coder: '💻', tester: '🧪', reviewer: '🔍', documenter: '📚',
  }
  return map[id] ?? '⚙️'
}
</script>

<style scoped>
.agents-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  overflow: hidden;
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

.agents-actions {
  display: flex;
  gap: 0.5rem;
}

.btn-primary, .btn-secondary, .btn-approve, .btn-reject, .btn-edit, .btn-add, .btn-icon {
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

.btn-primary:hover:not(:disabled) {
  opacity: 0.9;
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary, .btn-icon {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-fg);
}

.btn-secondary:hover, .btn-icon:hover {
  background: hsl(var(--muted-foreground) / 0.14);
}

.btn-approve { background: hsl(142 70% 45% / 0.15); color: hsl(142 70% 35%); }
.btn-reject { background: hsl(0 70% 45% / 0.15); color: hsl(0 70% 45%); }
.btn-edit { background: hsl(220 70% 50% / 0.15); color: hsl(220 70% 45%); }
.btn-add { background: transparent; color: var(--af-muted); border: 1px dashed var(--af-border); width: 100%; justify-content: center; }

.error-banner {
  padding: 0.5rem 1.25rem;
  background: hsl(0 70% 50% / 0.08);
  color: hsl(0 70% 45%);
  font-size: 0.88rem;
  border-bottom: 1px solid var(--af-border);
}

.agents-body {
  flex: 1;
  display: grid;
  grid-template-columns: 220px 1fr 180px;
  gap: 1px;
  background: var(--af-border);
  overflow: hidden;
}

.runs-sidebar, .pipeline-panel {
  background: var(--af-bg);
  overflow-y: auto;
  padding: 0.75rem;
}

.panel-title {
  font-size: 0.78rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--af-muted);
  margin-bottom: 0.5rem;
}

.empty-state {
  font-size: 0.88rem;
  color: var(--af-muted);
  text-align: center;
  padding: 1rem 0;
}

/* Run cards */
.run-card {
  position: relative;
  padding: 0.6rem;
  padding-bottom: 1.6rem;
  border-radius: 6px;
  border: 1px solid var(--af-border);
  margin-bottom: 0.5rem;
  cursor: pointer;
  transition: all 0.15s;
}

.run-card:hover, .run-card.active {
  border-color: hsl(var(--primary) / 0.3);
  background: hsl(var(--primary) / 0.03);
}

.run-card-header {
  display: flex;
  justify-content: space-between;
  align-items: flex-start;
  gap: 0.5rem;
  margin-bottom: 0.3rem;
}

.btn-delete {
  position: absolute;
  bottom: 0.4rem;
  right: 0.5rem;
  color: hsl(0 70% 50%);
  opacity: 0;
  transition: opacity 0.15s;
  z-index: 2;
}

.run-card:hover .btn-delete {
  opacity: 1;
}

.run-id {
  font-size: 0.83rem;
  font-weight: 500;
  color: var(--af-fg);
  font-family: 'JetBrains Mono', monospace;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.run-card-meta {
  display: flex;
  justify-content: space-between;
  font-size: 0.78rem;
  color: var(--af-muted);
  margin-bottom: 0.4rem;
}

.run-progress-bar {
  height: 4px;
  background: hsl(var(--muted-foreground) / 0.08);
  border-radius: 2px;
  overflow: hidden;
}

.run-progress-fill {
  height: 100%;
  background: var(--af-primary);
  border-radius: 2px;
  transition: width 0.3s ease;
}

/* Pipeline */
.run-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 0.75rem;
}

.run-title {
  font-size: 0.93rem;
  font-weight: 600;
  color: var(--af-fg);
  font-family: 'JetBrains Mono', monospace;
}

.run-stats {
  display: flex;
  gap: 0.5rem;
}

.stat-badge {
  display: inline-flex;
  align-items: center;
  gap: 0.25rem;
  font-size: 0.78rem;
  padding: 0.25rem 0.5rem;
  border-radius: 4px;
  background: hsl(var(--muted-foreground) / 0.06);
  color: var(--af-muted);
}

/* Budget bar */
.budget-bar-container {
  margin-bottom: 1rem;
}

.budget-label {
  display: flex;
  justify-content: space-between;
  font-size: 0.78rem;
  color: var(--af-muted);
  margin-bottom: 0.3rem;
}

.budget-bar {
  height: 6px;
  background: hsl(var(--muted-foreground) / 0.08);
  border-radius: 3px;
  overflow: hidden;
}

.budget-fill {
  height: 100%;
  background: hsl(142 70% 45%);
  border-radius: 3px;
  transition: width 0.3s ease;
}

.budget-fill.warning { background: hsl(38 90% 50%); }
.budget-fill.danger { background: hsl(0 70% 50%); }

/* Pipeline flow */
.pipeline-flow {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  padding: 1rem;
  border: 1px solid var(--af-border);
  border-radius: 8px;
  overflow-x: auto;
  margin-bottom: 1rem;
}

.pipeline-step {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: 0.2rem;
  padding: 0.5rem 0.6rem;
  border-radius: 8px;
  min-width: 72px;
  border: 1px solid transparent;
  transition: all 0.2s;
  position: relative;
  cursor: pointer;
}

.pipeline-step.completed {
  border-color: hsl(142 70% 45% / 0.25);
  background: hsl(142 70% 45% / 0.04);
}

.pipeline-step.running {
  border-color: hsl(var(--af-agents) / 0.4);
  background: hsl(var(--af-agents) / 0.08);
}

.pipeline-step.waiting_gate {
  border-color: hsl(38 90% 50% / 0.4);
  background: hsl(38 90% 50% / 0.08);
}

.pipeline-step.pending {
  opacity: 0.5;
}

.step-name { font-size: 0.73rem; font-weight: 500; color: var(--af-fg); }
.step-gate { font-size: 0.68rem; position: absolute; top: 2px; right: 2px; }
.step-pulse {
  position: absolute;
  top: 2px; left: 2px;
  width: 6px; height: 6px;
  border-radius: 50%;
  background: hsl(var(--af-agents));
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.4; transform: scale(1.3); }
  100% { opacity: 1; transform: scale(1); }
}

.step-connector {
  color: var(--af-border);
  display: flex;
  align-items: center;
}

/* Gate panel */
.gate-panel {
  padding: 0.75rem 1rem;
  border: 1px solid hsl(38 90% 50% / 0.3);
  border-radius: 8px;
  background: hsl(38 90% 50% / 0.04);
  margin-bottom: 1rem;
}

.gate-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.88rem;
  font-weight: 500;
  color: hsl(38 80% 35%);
  margin-bottom: 0.5rem;
}

.gate-actions {
  display: flex;
  gap: 0.4rem;
}

/* History */
.history-panel {
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.75rem 1rem;
}

.history-row {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: 0.3rem 0;
  border-bottom: 1px solid var(--af-border);
  font-size: 0.88rem;
}

.history-row:last-child { border-bottom: none; }
.history-profession { font-weight: 500; color: var(--af-fg); }
.history-time { color: var(--af-muted); font-family: monospace; font-size: 0.83rem; }

/* Expanded step card */
.expanded-step-card {
  position: absolute;
  top: calc(100% + 8px);
  left: 50%;
  transform: translateX(-50%);
  width: 200px;
  background: var(--af-card);
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.6rem 0.75rem;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.08);
  z-index: 20;
}

.expanded-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-bottom: 0.4rem;
}

.expanded-name {
  flex: 1;
  font-size: 0.88rem;
  font-weight: 600;
  color: var(--af-fg);
}

.expanded-metrics {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  margin-bottom: 0.4rem;
}

.metric {
  display: flex;
  justify-content: space-between;
  align-items: center;
  font-size: 0.83rem;
}

.metric-label { color: var(--af-muted); }
.metric-value { color: var(--af-fg); font-weight: 500; }

.expanded-actions {
  display: flex;
  justify-content: center;
}

.expanded-hint {
  font-size: 0.73rem;
  color: var(--af-muted);
}

/* Step retry badge */
.step-retry {
  position: absolute;
  bottom: 2px;
  right: 2px;
  font-size: 0.68rem;
  font-weight: 600;
  color: hsl(var(--af-error));
  background: hsl(var(--af-error) / 0.1);
  padding: 0 0.2rem;
  border-radius: 3px;
}

/* Session Log */
.session-log-panel {
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.75rem 1rem;
  margin-bottom: 1rem;
  display: flex;
  flex-direction: column;
  max-height: 380px;
}

.session-log-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  overflow-y: auto;
  padding-right: 0.25rem;
}

.session-entry {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
  padding: 0.4rem 0.5rem;
  border-radius: 6px;
  background: hsl(var(--muted-foreground) / 0.03);
}

.session-entry-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  font-size: 0.78rem;
}

.session-profession {
  font-weight: 600;
  color: var(--af-fg);
  text-transform: capitalize;
}

.session-time {
  color: var(--af-muted);
  font-family: monospace;
  font-size: 0.72rem;
  margin-left: auto;
}

.session-text pre {
  margin: 0;
  padding: 0.3rem 0.4rem;
  background: hsl(var(--muted-foreground) / 0.04);
  border-radius: 4px;
  font-size: 0.82rem;
  line-height: 1.5;
  color: var(--af-fg);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 200px;
  overflow-y: auto;
}

.session-tool {
  border: 1px solid hsl(var(--primary) / 0.2);
  border-radius: 4px;
  background: hsl(var(--primary) / 0.04);
}

.session-tool.result {
  border-color: hsl(150 60% 45% / 0.3);
  background: hsl(150 60% 45% / 0.06);
}

.tool-header {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.3rem 0.5rem;
  font-size: 0.78rem;
  font-weight: 600;
  color: hsl(var(--primary));
}

.session-tool.result .tool-header {
  color: hsl(150 60% 35%);
}

.tool-details {
  padding: 0 0.5rem 0.4rem;
}

.tool-details summary {
  font-size: 0.72rem;
  color: var(--af-muted);
  cursor: pointer;
  user-select: none;
}

.tool-details pre {
  margin: 0.25rem 0 0;
  padding: 0.3rem 0.4rem;
  background: hsl(var(--muted-foreground) / 0.05);
  border-radius: 4px;
  font-size: 0.75rem;
  line-height: 1.4;
  max-height: 120px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.session-complete {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: hsl(150 60% 35%);
}

.session-error {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: hsl(0 72% 51%);
  background: hsl(0 72% 51% / 0.06);
  padding: 0.3rem 0.5rem;
  border-radius: 4px;
}

.session-warning {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: hsl(38 92% 50%);
  background: hsl(38 92% 50% / 0.08);
  padding: 0.3rem 0.5rem;
  border-radius: 4px;
}

.session-step {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: hsl(var(--af-agents));
  background: hsl(var(--af-agents) / 0.08);
  padding: 0.3rem 0.5rem;
  border-radius: 4px;
}

.session-step.completed {
  color: hsl(150 60% 35%);
  background: hsl(150 60% 35% / 0.08);
}

/* Cost Breakdown */
.cost-panel {
  border: 1px solid var(--af-border);
  border-radius: 8px;
  padding: 0.75rem 1rem;
  margin-bottom: 1rem;
}

.cost-bars {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.cost-bar-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.83rem;
}

.cost-label {
  width: 50px;
  color: var(--af-muted);
  flex-shrink: 0;
}

.cost-bar-bg {
  flex: 1;
  height: 6px;
  background: hsl(var(--muted-foreground) / 0.08);
  border-radius: 3px;
  overflow: hidden;
}

.cost-bar-fill {
  height: 100%;
  background: var(--af-primary);
  border-radius: 3px;
  transition: width 0.3s ease;
}

.cost-bar-fill.budget {
  background: hsl(var(--af-success));
}

.cost-value {
  width: 40px;
  text-align: right;
  color: var(--af-fg);
  font-weight: 500;
  flex-shrink: 0;
}

/* ─── Mobile Responsive ───────────────────────────────────────────────────── */

@media (max-width: 768px) {
  .agents-body {
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr auto;
    overflow-y: auto;
  }

  .runs-sidebar {
    max-height: 180px;
    border-bottom: 1px solid var(--af-border);
  }

  .pipeline-flow {
    padding: 0.5rem;
  }

  .expanded-step-card {
    position: static;
    transform: none;
    width: 100%;
    margin: 0.5rem 0;
  }
}

/* Modal */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.3);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}

.modal-content {
  background: var(--af-card);
  border: 1px solid var(--af-border);
  border-radius: 10px;
  padding: 1.25rem;
  width: 480px;
  max-width: 90vw;
  max-height: 80vh;
  overflow-y: auto;
}

.modal-content h3 {
  font-size: 0.98rem;
  font-weight: 600;
  margin-bottom: 1rem;
  color: var(--af-fg);
}

.form-group {
  margin-bottom: 0.75rem;
}

.form-group label {
  display: block;
  font-size: 0.83rem;
  font-weight: 500;
  color: var(--af-muted);
  margin-bottom: 0.3rem;
}

.form-group input, .form-group select, .form-group textarea {
  width: 100%;
  padding: 0.4rem 0.5rem;
  border: 1px solid var(--af-border);
  border-radius: 5px;
  background: var(--af-bg);
  color: var(--af-fg);
  font-size: 0.88rem;
  font-family: inherit;
}

.form-group textarea {
  resize: vertical;
}

.steps-builder {
  display: flex;
  flex-direction: column;
  gap: 0.4rem;
}

.step-row {
  display: flex;
  gap: 0.4rem;
  align-items: center;
}

.step-input { flex: 1; }
.step-select { width: 100px; }

.modal-actions {
  display: flex;
  justify-content: flex-end;
  gap: 0.5rem;
  margin-top: 1rem;
}
</style>
