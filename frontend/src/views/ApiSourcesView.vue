<template>
  <div class="api-sources-view">
    <div class="sources-sidebar">
      <div class="sidebar-header">
        <h2>API Sources</h2>
        <button class="btn-add" @click="startCreate" title="Add API Source">
          <Plus :size="16" />
        </button>
      </div>

      <div v-if="loading" class="sidebar-empty">Loading...</div>

      <div v-else-if="!hasSources" class="sidebar-empty">
        <p>No API sources configured.</p>
        <button class="btn-scan" @click="handleScan" :disabled="scanning">
          <Search :size="14" />
          {{ scanning ? 'Scanning...' : 'Scan for Configs' }}
        </button>
        <button class="btn-add-small" @click="startCreate">
          <Plus :size="14" /> Add Manually
        </button>
      </div>

      <div v-else class="source-list">
        <button
          v-for="source in sources"
          :key="source.id"
          class="source-card"
          :class="{ active: selectedId === source.id }"
          @click="selectSource(source.id)"
        >
          <div class="source-icon" :class="source.provider">
            <component :is="providerIcon(source.provider)" :size="18" />
          </div>
          <div class="source-info">
            <div class="source-name">{{ source.name }}</div>
            <div class="source-meta">
              <span class="status-dot" :class="source.is_available ? 'ok' : 'err'" />
              {{ source.models.length }} models
            </div>
          </div>
        </button>
      </div>
    </div>

    <div class="sources-detail">
      <template v-if="editing">
        <div class="detail-header">
          <h3>{{ isNew ? 'New API Source' : editing.name }}</h3>
          <div class="detail-actions">
            <button v-if="!isNew" class="btn-test" @click="handleTest" :disabled="testing">
              <Zap :size="14" />
              {{ testing ? 'Testing...' : 'Test Connection' }}
            </button>
            <button class="btn-save" @click="handleSave" :disabled="saving">
              {{ saving ? 'Saving...' : 'Save' }}
            </button>
            <button v-if="!isNew" class="btn-delete" @click="handleDelete">
              <Trash2 :size="14" />
            </button>
            <button class="btn-cancel" @click="cancelEdit">Cancel</button>
          </div>
        </div>

        <div v-if="testResult" class="test-banner" :class="testResult.success ? 'ok' : 'err'">
          <span v-if="testResult.success">Connected to {{ testResult.model }} ({{ testResult.latency_ms }}ms)</span>
          <span v-else>{{ testResult.error }}</span>
        </div>

        <div class="detail-form">
          <div class="form-group">
            <label>Provider</label>
            <select v-model="editing.provider" :disabled="!isNew" class="form-select">
              <option value="anthropic">Anthropic (Claude)</option>
              <option value="openai">OpenAI (GPT)</option>
              <option value="local">Local / Ollama</option>
            </select>
          </div>

          <div class="form-group">
            <label>Name</label>
            <input v-model="editing.name" class="form-input" placeholder="My API Source" />
          </div>

          <div class="form-group">
            <label>API Key</label>
            <div class="key-input">
              <input
                v-model="apiKeyInput"
                class="form-input"
                :type="showKey ? 'text' : 'password'"
                placeholder="Enter API key"
              />
              <button class="btn-icon" @click="showKey = !showKey" title="Toggle visibility">
                <Eye v-if="!showKey" :size="14" />
                <EyeOff v-else :size="14" />
              </button>
            </div>
            <div class="form-hint">Key is stored locally (base64-encoded). You can also use env vars.</div>
          </div>

          <div class="form-group">
            <label>API Key Env Var <span class="optional">(optional)</span></label>
            <input v-model="editing.api_key_env" class="form-input" placeholder="ANTHROPIC_API_KEY or settings:KEY" />
          </div>

          <div v-if="editing.provider === 'local'" class="form-group">
            <label>Base URL</label>
            <input v-model="editing.base_url" class="form-input" placeholder="http://localhost:11434" />
          </div>

          <div class="form-group">
            <label>Models</label>
            <div class="models-table">
              <div class="model-header">
                <span class="col-id">Model ID</span>
                <span class="col-name">Display Name</span>
                <span class="col-tier">Tier</span>
                <span class="col-action" />
              </div>
              <div v-for="(model, i) in editing.models" :key="i" class="model-row">
                <input v-model="model.id" class="model-input" placeholder="model-id" />
                <input v-model="model.name" class="model-input" placeholder="Display Name" />
                <select v-model="model.tier" class="model-select">
                  <option value="light">Light</option>
                  <option value="mid">Mid</option>
                  <option value="heavy">Heavy</option>
                </select>
                <button class="btn-icon btn-remove" @click="editing.models.splice(i, 1)">
                  <X :size="12" />
                </button>
              </div>
              <button class="btn-add-model" @click="editing.models.push({ id: '', name: '', tier: 'mid' as const })">
                <Plus :size="12" /> Add Model
              </button>
            </div>
          </div>
        </div>
      </template>

      <div v-else-if="scanResults.length" class="detail-scan">
        <div class="scan-header">
          <Search :size="20" />
          <h3>Import API Sources</h3>
        </div>
        <p class="scan-hint">Found existing configurations on your system. Select which to import.</p>
        <div class="scan-list">
          <label v-for="candidate in scanResults" :key="candidate.id" class="scan-item">
            <input type="checkbox" :value="candidate.id" v-model="selectedImports" />
            <div class="scan-item-icon" :class="candidate.provider">
              <component :is="providerIcon(candidate.provider)" :size="18" />
            </div>
            <div class="scan-item-info">
              <div class="scan-item-name">{{ candidate.name }}</div>
              <div class="scan-item-models">{{ candidate.models.length }} models detected</div>
            </div>
          </label>
        </div>
        <div class="scan-actions">
          <button class="btn-primary" @click="handleImport" :disabled="!selectedImports.length || importing">
            {{ importing ? 'Importing...' : 'Import Selected' }}
          </button>
          <button class="btn-cancel" @click="scanResults = []">Cancel</button>
        </div>
      </div>

      <div v-else class="detail-empty">
        <Server :size="48" />
        <p>Select an API source or add a new one</p>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, onMounted, computed } from 'vue'
import {
  Plus, X, Zap, Trash2, Eye, EyeOff, Server, Cloud, Cpu, Search,
} from 'lucide-vue-next'
import { useApiSources, type ApiSource, type ModelTier } from '@/composables/useApiSources'

const {
  sources, loading, hasSources, testResult,
  loadSources, createSource, updateSource, deleteSource, testConnection,
  scanSources, importSources,
} = useApiSources()

const selectedId = ref<string | null>(null)
const editing = ref<ApiSource | null>(null)
const isNew = ref(false)
const saving = ref(false)
const testing = ref(false)
const showKey = ref(false)
const apiKeyInput = ref('')
const scanning = ref(false)
const importing = ref(false)
const scanResults = ref<ApiSource[]>([])
const selectedImports = ref<string[]>([])

const providerIcon = (provider: string) => {
  switch (provider) {
    case 'anthropic': return Cloud
    case 'openai': return Cloud
    case 'local': return Cpu
    default: return Server
  }
}

function selectSource(id: string) {
  selectedId.value = id
  const source = sources.value.find(s => s.id === id)
  if (source) {
    editing.value = JSON.parse(JSON.stringify(source))
    isNew.value = false
    apiKeyInput.value = ''
  }
}

function startCreate() {
  editing.value = {
    id: `source-${Date.now()}`,
    name: '',
    provider: 'anthropic',
    api_key_env: '',
    api_key_stored: null,
    base_url: null,
    is_available: false,
    models: [
      { id: 'claude-3-5-haiku-20241022', name: 'Claude 3.5 Haiku', tier: 'light' as ModelTier },
      { id: 'claude-3-5-sonnet-20241022', name: 'Claude 3.5 Sonnet', tier: 'mid' as ModelTier },
      { id: 'claude-3-opus-20240229', name: 'Claude 3 Opus', tier: 'heavy' as ModelTier },
    ],
  }
  isNew.value = true
  selectedId.value = null
  apiKeyInput.value = ''
}

function cancelEdit() {
  editing.value = null
  selectedId.value = null
}

async function handleSave() {
  if (!editing.value) return
  saving.value = true

  const source = { ...editing.value }
  if (apiKeyInput.value) {
    source.api_key_stored = apiKeyInput.value
  }

  if (isNew.value) {
    const ok = await createSource(source)
    if (ok) {
      selectedId.value = source.id
      isNew.value = false
    }
  } else {
    await updateSource(source.id, source)
  }
  saving.value = false
}

async function handleDelete() {
  if (!editing.value) return
  if (!confirm('Delete this API source?')) return
  const ok = await deleteSource(editing.value.id)
  if (ok) {
    editing.value = null
    selectedId.value = null
  }
}

async function handleTest() {
  if (!editing.value) return
  testing.value = true
  await testConnection(editing.value.id)
  testing.value = false
}

async function handleScan() {
  scanning.value = true
  const results = await scanSources()
  scanResults.value = results
  selectedImports.value = results.map(s => s.id)
  scanning.value = false
}

async function handleImport() {
  if (!selectedImports.value.length) return
  importing.value = true
  const ok = await importSources(selectedImports.value)
  if (ok) {
    scanResults.value = []
    selectedImports.value = []
  }
  importing.value = false
}

onMounted(loadSources)
</script>

<style scoped>
.api-sources-view {
  display: flex;
  height: 100%;
  overflow: hidden;
}

.sources-sidebar {
  width: 280px;
  border-right: 1px solid var(--af-border);
  display: flex;
  flex-direction: column;
  flex-shrink: 0;
  background: hsl(var(--secondary));
}

.sidebar-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 1rem 0.5rem;
}

.sidebar-header h2 {
  font-size: 0.98rem;
  font-weight: 600;
  color: var(--af-fg);
}

.sidebar-empty {
  padding: 1.5rem 1rem;
  text-align: center;
  color: var(--af-muted);
  font-size: 0.88rem;
}

.sidebar-empty .hint {
  margin: 0.5rem 0 1rem;
  font-size: 0.83rem;
}

.source-list {
  flex: 1;
  overflow-y: auto;
  padding: 0 0.5rem 0.5rem;
}

.source-card {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  width: 100%;
  padding: 0.6rem 0.7rem;
  background: transparent;
  border: 1px solid transparent;
  border-radius: 6px;
  color: var(--af-fg);
  cursor: pointer;
  transition: all 0.15s;
  text-align: left;
  margin-bottom: 2px;
}

.source-card:hover {
  background: hsl(var(--muted-foreground) / 0.06);
}

.source-card.active {
  background: hsl(var(--primary) / 0.08);
  border-color: hsl(var(--primary) / 0.2);
}

.source-icon {
  width: 32px;
  height: 32px;
  border-radius: 6px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.source-icon.anthropic { background: hsl(25 80% 50% / 0.12); color: hsl(25 80% 50%); }
.source-icon.openai { background: hsl(160 60% 45% / 0.12); color: hsl(160 60% 45%); }
.source-icon.local { background: hsl(220 60% 50% / 0.12); color: hsl(220 60% 50%); }

.source-name {
  font-size: 0.88rem;
  font-weight: 500;
}

.source-meta {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  font-size: 0.78rem;
  color: var(--af-muted);
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
}

.status-dot.ok { background: hsl(var(--success)); }
.status-dot.err { background: hsl(var(--error)); }

/* Detail panel */
.sources-detail {
  flex: 1;
  overflow-y: auto;
  padding: 1rem 1.5rem;
}

.detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 1rem;
  padding-bottom: 0.75rem;
  border-bottom: 1px solid var(--af-border);
}

.detail-header h3 {
  font-size: 1rem;
  font-weight: 600;
}

.detail-actions {
  display: flex;
  gap: 0.4rem;
}

.detail-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--af-muted);
  gap: 0.75rem;
}

/* Test banner */
.test-banner {
  padding: 0.5rem 0.75rem;
  border-radius: 6px;
  font-size: 0.88rem;
  margin-bottom: 1rem;
}

.test-banner.ok {
  background: hsl(var(--success) / 0.1);
  color: hsl(var(--success));
  border: 1px solid hsl(var(--success) / 0.2);
}

.test-banner.err {
  background: hsl(var(--error) / 0.1);
  color: hsl(var(--error));
  border: 1px solid hsl(var(--error) / 0.2);
}

/* Form */
.detail-form {
  display: flex;
  flex-direction: column;
  gap: 1rem;
  max-width: 600px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
}

.form-group label {
  font-size: 0.88rem;
  font-weight: 500;
  color: var(--af-fg);
}

.optional {
  font-weight: 400;
  color: var(--af-muted);
}

.form-input, .form-select {
  padding: 0.45rem 0.6rem;
  border: 1px solid var(--af-border);
  border-radius: 5px;
  background: var(--af-card);
  color: var(--af-fg);
  font-size: 0.88rem;
  outline: none;
  transition: border-color 0.15s;
}

.form-input:focus, .form-select:focus {
  border-color: var(--af-primary);
}

.form-hint {
  font-size: 0.78rem;
  color: var(--af-muted);
}

.key-input {
  display: flex;
  gap: 0.3rem;
}

.key-input .form-input {
  flex: 1;
}

/* Models table */
.models-table {
  border: 1px solid var(--af-border);
  border-radius: 6px;
  overflow: hidden;
}

.model-header {
  display: grid;
  grid-template-columns: 1fr 1fr 100px 28px;
  gap: 0.3rem;
  padding: 0.4rem 0.5rem;
  background: hsl(var(--secondary));
  font-size: 0.78rem;
  font-weight: 600;
  color: var(--af-muted);
  text-transform: uppercase;
}

.model-row {
  display: grid;
  grid-template-columns: 1fr 1fr 100px 28px;
  gap: 0.3rem;
  padding: 0.3rem 0.5rem;
  border-top: 1px solid var(--af-border);
}

.model-input, .model-select {
  padding: 0.3rem 0.4rem;
  border: 1px solid var(--af-border);
  border-radius: 4px;
  background: var(--af-card);
  color: var(--af-fg);
  font-size: 0.83rem;
  outline: none;
}

.model-input:focus, .model-select:focus {
  border-color: var(--af-primary);
}

.btn-add-model {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  width: 100%;
  padding: 0.4rem 0.5rem;
  border: none;
  border-top: 1px solid var(--af-border);
  background: transparent;
  color: var(--af-primary);
  font-size: 0.83rem;
  cursor: pointer;
}

.btn-add-model:hover {
  background: hsl(var(--primary) / 0.06);
}

/* Buttons */
.btn-add, .btn-primary, .btn-save, .btn-test, .btn-delete, .btn-cancel, .btn-icon {
  border: none;
  border-radius: 5px;
  cursor: pointer;
  font-size: 0.83rem;
  transition: all 0.15s;
}

.btn-add {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  background: hsl(var(--primary) / 0.1);
  color: var(--af-primary);
}

.btn-add:hover {
  background: hsl(var(--primary) / 0.2);
}

.btn-primary {
  padding: 0.4rem 0.8rem;
  background: var(--af-primary);
  color: #fff;
}

.btn-save {
  padding: 0.35rem 0.7rem;
  background: var(--af-primary);
  color: #fff;
  font-weight: 500;
}

.btn-save:disabled { opacity: 0.6; }

.btn-test {
  display: flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.35rem 0.7rem;
  background: transparent;
  border: 1px solid var(--af-border);
  color: var(--af-fg);
}

.btn-test:disabled { opacity: 0.6; }

.btn-delete {
  display: flex;
  align-items: center;
  padding: 0.35rem 0.5rem;
  background: transparent;
  color: hsl(var(--error));
}

.btn-delete:hover {
  background: hsl(var(--error) / 0.1);
}

.btn-cancel {
  padding: 0.35rem 0.6rem;
  background: transparent;
  border: 1px solid var(--af-border);
  color: var(--af-muted);
}

.btn-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  background: transparent;
  color: var(--af-muted);
  border: 1px solid var(--af-border);
}

.btn-icon:hover {
  background: hsl(var(--muted-foreground) / 0.06);
}

.btn-remove {
  border: none;
  color: var(--af-muted);
}

.btn-remove:hover {
  color: hsl(var(--error));
}

/* Scan / Import */
.btn-scan {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.4rem;
  width: 100%;
  padding: 0.5rem;
  border: 1px dashed var(--af-border);
  border-radius: 6px;
  background: transparent;
  color: var(--af-primary);
  font-size: 0.88rem;
  cursor: pointer;
  transition: all 0.15s;
  margin-bottom: 0.4rem;
}

.btn-scan:hover {
  background: hsl(var(--primary) / 0.06);
  border-color: var(--af-primary);
}

.btn-scan:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.btn-add-small {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: 0.3rem;
  width: 100%;
  padding: 0.4rem;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--af-muted);
  font-size: 0.83rem;
  cursor: pointer;
}

.btn-add-small:hover {
  color: var(--af-fg);
}

.detail-scan {
  max-width: 500px;
  padding-top: 2rem;
}

.scan-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  color: var(--af-primary);
  margin-bottom: 0.5rem;
}

.scan-header h3 {
  font-size: 1rem;
  font-weight: 600;
  color: var(--af-fg);
}

.scan-hint {
  font-size: 0.88rem;
  color: var(--af-muted);
  margin-bottom: 1rem;
}

.scan-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
  margin-bottom: 1rem;
}

.scan-item {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  padding: 0.75rem;
  border: 1px solid var(--af-border);
  border-radius: 8px;
  cursor: pointer;
  transition: all 0.15s;
}

.scan-item:hover {
  border-color: hsl(var(--primary) / 0.3);
  background: hsl(var(--primary) / 0.04);
}

.scan-item input[type="checkbox"] {
  accent-color: var(--af-primary);
}

.scan-item-icon {
  width: 36px;
  height: 36px;
  border-radius: 8px;
  display: flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}

.scan-item-icon.anthropic { background: hsl(25 80% 50% / 0.12); color: hsl(25 80% 50%); }
.scan-item-icon.openai { background: hsl(160 60% 45% / 0.12); color: hsl(160 60% 45%); }
.scan-item-icon.local { background: hsl(220 60% 50% / 0.12); color: hsl(220 60% 50%); }

.scan-item-name {
  font-size: 0.93rem;
  font-weight: 500;
}

.scan-item-models {
  font-size: 0.78rem;
  color: var(--af-muted);
}

.scan-actions {
  display: flex;
  gap: 0.5rem;
}
</style>
