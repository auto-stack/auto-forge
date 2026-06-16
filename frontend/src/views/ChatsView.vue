<template>
  <div class="chats-view">
    <!-- Session Sidebar -->
    <aside class="session-sidebar" :class="{ collapsed: sidebarCollapsed }">
      <div class="sidebar-header">
        <span class="sidebar-title">{{ t('chat.sessions') }}</span>
        <button class="sidebar-new-btn" @click="clearSession(projectPath ?? undefined)" :title="t('chat.newSession')">
          <Plus :size="14" />
        </button>
        <button class="sidebar-delete-all-btn" @click="confirmDeleteAll" :title="t('chat.deleteAllSessions')" v-if="sessionList.length > 0">
          <Trash2 :size="14" />
        </button>
        <button class="sidebar-collapse-btn" @click="sidebarCollapsed = !sidebarCollapsed" :title="t('chat.toggleSidebar')">
          <PanelLeft :size="14" />
        </button>
      </div>
      <div class="session-list">
        <div
          v-for="s in sessionList"
          :key="s.id"
          class="session-item"
          :class="{ active: sessionId === s.id }"
          @click="switchSession(s.id)"
        >
          <div v-if="editingSessionId !== s.id" class="session-preview">{{ s.name || s.preview || t('chat.newSession') }}</div>
          <input
            v-else
            :data-rename-input="s.id"
            v-model="editingName"
            class="session-rename-input"
            @keydown.enter="commitRename"
            @keydown.escape="cancelRename"
            @blur="commitRename"
            @click.stop
          />
          <div class="session-meta">
            <span class="session-count">{{ t('chat.msgs', { count: s.message_count }) }}</span>
            <button
              v-if="editingSessionId !== s.id"
              class="session-rename-btn"
              :title="t('chat.renameSession')"
              @click.stop="startRename(s)"
            >
              <Pencil :size="11" />
            </button>
            <button
              class="session-delete-btn"
              :title="t('chat.deleteSession')"
              @click.stop="confirmDelete(s.id)"
            >
              <Trash2 :size="12" />
            </button>
          </div>
        </div>
        <div v-if="sessionList.length === 0" class="session-empty">
          {{ t('chat.noSessions') }}
        </div>
      </div>
    </aside>

    <!-- Main Chat Area -->
    <div class="chats-body">
      <div class="chats-header">
        <div class="header-title-row">
          <button v-if="sidebarCollapsed" class="sidebar-toggle-btn" @click="sidebarCollapsed = false" :title="t('chat.showSessions')">
            <PanelLeft :size="16" />
          </button>
          <h2>{{ t('chat.title') }}</h2>
        </div>
        <div class="header-center">
          <div class="header-search">
            <Search :size="13" />
            <input
              ref="searchInputRef"
              v-model="chatSearch"
              type="text"
              class="search-input"
              :placeholder="t('chat.searchPlaceholder')"
            />
          </div>
        </div>
        <div class="header-actions">
          <button
            v-if="Object.keys(relayRuns).length > 0"
            class="errand-toggle-btn relay-toggle-btn"
            @click="openRelayView"
          >
            <span class="errand-toggle-label">{{ t('chat.relay') }}</span>
            <span class="errand-toggle-badge">{{ Object.keys(relayRuns).length }}</span>
          </button>
          <button
            v-if="Object.keys(taskPlans).length > 0"
            class="errand-toggle-btn relay-toggle-btn"
            @click="openRelayView"
          >
            <span class="errand-toggle-label">{{ t('chat.taskPlans') }}</span>
            <span class="errand-toggle-badge">{{ Object.keys(taskPlans).length }}</span>
          </button>
          <button
            v-if="Object.keys(errands).length > 0"
            class="errand-toggle-btn"
            :title="allErrandsExpanded ? t('chat.collapseErrands') : t('chat.expandErrands')"
            @click="toggleAllErrands"
          >
            <span class="errand-toggle-label">{{ allErrandsExpanded ? t('chat.collapse') : t('chat.expand') }}</span>
            <span class="errand-toggle-badge">{{ Object.keys(errands).length }}</span>
          </button>
          <!-- Session info button -->
          <div class="session-info-wrapper">
            <button
              class="session-info-btn"
              :title="t('chat.sessionInfo')"
              @click="showSessionInfo = !showSessionInfo"
            >
              <Info :size="15" />
            </button>
            <div v-if="showSessionInfo" class="session-info-tooltip" @click.stop>
              <div class="session-info-row">
                <span class="session-info-label">{{ t('chat.chatId') }}</span>
                <code class="session-info-value session-info-id">{{ sessionId }}</code>
                <button class="session-info-copy" @click="copyChatId" :title="t('chat.copyChatId')">
                  <CopyCheck v-if="copiedChatId" :size="12" />
                  <Copy v-else :size="12" />
                </button>
              </div>
              <div class="session-info-row">
                <span class="session-info-label">{{ t('chat.messages') }}</span>
                <span class="session-info-value">{{ messages.length }}</span>
              </div>
              <div class="session-info-row">
                <span class="session-info-label">{{ t('chat.tokenCost') }}</span>
                <span class="session-info-value">{{ sessionTokenCost }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
      <div class="chat-canvas" ref="chatRef">
        <!-- Cross-session secretary gate -->
        <SecretaryMessage
          v-if="currentSecretary"
          :gate="currentSecretary"
          :queue-position="Math.max(0, gateBadgeCount - 1)"
          @approve="onSecretaryApprove"
          @reject="onSecretaryReject"
          @snooze="onSecretarySnooze"
          @review-in-specs="onReviewInSpecs"
        />

        <!-- Terminal report card -->
        <ReportCard
          v-if="reportData"
          :report="reportData"
          @view-full="onViewFullReport"
          @download="onDownloadReport"
          @open-files="onOpenChangedFiles"
        />

        <div class="chat-inner">
          <div
            v-for="msg in filteredMessages"
            v-show="msg.role !== 'tool'"
            :key="msg.id"
            class="message"
            :class="msg.role"
          >
            <div class="message-header">
              <AgentAvatar
                v-if="msg.role === 'assistant' && msg.profession_id"
                :profession-id="msg.profession_id"
                :name="agentConfigs.find(c => c.profession_id === msg.profession_id)?.name"
                size="sm"
              />
              <span class="role-badge" :class="msg.role">
                {{ msg.role === 'system' ? assistantName
                   : msg.role === 'assistant' && msg.profession_id
                     ? agentConfigs.find(c => c.profession_id === msg.profession_id)?.name ?? msg.profession_id
                     : msg.role }}
              </span>
              <span class="msg-time">{{ formatTime(msg.timestamp) }}</span>
            </div>
            <div class="message-content" :class="{ 'has-border': msg.role === 'assistant' && msg.content.length > 200 }">
              <span v-if="msg.role === 'assistant' && msg.content === '' && isStreamingMessage(msg)" class="typing-dots">
                <span></span><span></span><span></span>
              </span>
              <StreamingRenderer
                v-else-if="msg.role === 'assistant'"
                :source="questionnaireFor(msg)?.strippedContent ?? msg.content"
                :streaming="isStreamingMessage(msg)"
              />
              <div v-else-if="msg.role === 'system'" class="system-welcome">
                <span class="welcome-icon">👋</span>
                <span>{{ t('chat.welcomeText', { name: assistantName }) }}</span>
              </div>
              <div v-else-if="msg.content" class="user-text" v-html="renderMentions(msg.content)"></div>
            </div>
            <!-- Thinking block -->
            <div v-if="msg.role === 'assistant' && msg.thinking" class="thinking-block">
              <details :open="isStreamingMessage(msg) && msg.content === ''">
                <summary>
                  <span class="thinking-icon">💭</span>
                  <span class="thinking-label">思考中</span>
                  <span v-if="isStreamingMessage(msg)" class="thinking-pulse"></span>
                </summary>
                <pre class="thinking-content">{{ msg.thinking }}</pre>
              </details>
            </div>
            <QuestionnaireCard
              v-if="msg.role === 'assistant' && questionnaireFor(msg)?.questions"
              :questions="questionnaireFor(msg)!.questions"
              @submit="(answers: AnswersMap) => onQuestionnaireSubmit(answers, msg)"
            />
            <div v-if="msg.tool_calls && msg.tool_calls.length > 0" class="tool-calls">
              <template v-for="tc in msg.tool_calls" :key="tc.id">
                <!-- Handoff card for bring_in -->
                <div v-if="tc.name === 'bring_in' && tc.result" class="handoff-card">
                  <div class="handoff-flow">
                    <span class="handoff-agent">{{ professionDisplayName(msg.profession_id || 'assistant') }}</span>
                    <span class="handoff-arrow">→</span>
                    <span class="handoff-agent target">{{ getHandoffTarget(tc) }}</span>
                  </div>
                  <div class="handoff-reason">{{ handoffReason(tc) }}</div>
                  <span v-if="tc.arguments?.classification" class="handoff-badge">{{ tc.arguments.classification }}</span>
                </div>
                <!-- Shell card -->
                <div v-else-if="tc.name === 'shell'" class="shell-card" :class="tc.status">
                  <div class="shell-header">
                    <span class="shell-icon">$</span>
                    <code class="shell-cmd">{{ tc.arguments?.command || '' }}</code>
                    <span class="shell-status" :class="tc.status">{{ tc.status }}</span>
                  </div>
                  <pre v-if="tc.result && tc._expanded" class="shell-output">{{ tc.result }}</pre>
                  <button v-if="tc.result && !tc._expanded" class="shell-toggle" @click="tc._expanded = true">show output</button>
                  <button v-if="tc.result && tc._expanded" class="shell-toggle" @click="tc._expanded = false">hide</button>
                </div>
                <!-- Spawn Relay card -->
                <div v-else-if="tc.name === 'spawn_relay'" class="relay-card" :class="tc.status">
                  <div class="relay-header">
                    <span class="relay-icon">🚀</span>
                    <span class="relay-name">Relay: {{ tc.arguments?.flow_id || 'standard' }}</span>
                    <span class="relay-status" :class="getRelayStatus(tc)?.status || 'started'">
                      {{ getRelayStatus(tc)?.status || 'started' }}
                    </span>
                    <button v-if="getRelayStatus(tc)?.run_id" class="relay-view-btn" @click="goToRelayRun(getRelayStatus(tc)!.run_id)">
                      Monitor →
                    </button>
                  </div>
                  <div v-if="tc.arguments?.task" class="relay-task">{{ tc.arguments.task }}</div>
                  <div v-if="getRelayStatus(tc)?.summary" class="relay-summary">{{ getRelayStatus(tc)?.summary }}</div>
                </div>
                <!-- Dispatch / Errand card -->
                <div v-else-if="tc.name === 'dispatch'" class="errand-card" :class="tc.status">
                  <div class="errand-header" @click="tc._expanded = !tc._expanded">
                    <span class="errand-icon">🔍</span>
                    <span class="errand-name">Errand: {{ getErrandTask(tc) }}</span>
                    <span class="errand-status" :class="getErrandStatus(tc)?.status || 'running'">
                      {{ getErrandStatus(tc)?.status || 'running' }}
                    </span>
                    <span v-if="getErrandStatus(tc)?.token_usage" class="errand-cost">
                      {{ getErrandStatus(tc)?.token_usage }} tok
                    </span>
                    <ChevronDown v-if="!tc._expanded" :size="14" class="tool-chevron" />
                    <ChevronUp v-else :size="14" class="tool-chevron" />
                  </div>
                  <div v-if="tc._expanded" class="errand-body">
                    <div class="errand-task">{{ getErrandTask(tc) }}</div>
                    <!-- Live errand content -->
                    <div v-if="getErrandContent(tc)" class="errand-content">
                      <StreamingRenderer :source="getErrandContent(tc)" :streaming="getErrandStatus(tc)?.status === 'running'" />
                    </div>
                    <!-- Errand tool calls -->
                    <div v-if="getErrandToolCalls(tc).length > 0" class="errand-tool-calls">
                      <div v-for="(etc, i) in getErrandToolCalls(tc)" :key="i" class="errand-sub-tool">
                        <div class="errand-sub-tool-header">
                          <span class="errand-sub-tool-name">{{ etc.name }}</span>
                          <span class="errand-sub-tool-status" :class="etc.status">{{ etc.status }}</span>
                        </div>
                        <pre v-if="etc.result" class="errand-sub-tool-result">{{ etc.result }}</pre>
                      </div>
                    </div>
                    <!-- Final result -->
                    <div v-if="getErrandStatus(tc)?.result && getErrandStatus(tc)?.status !== 'running'" class="errand-result">
                      <div class="errand-result-label">Result</div>
                      <pre class="errand-result-text">{{ getErrandStatus(tc)?.result }}</pre>
                    </div>
                  </div>
                </div>
                <!-- Generic tool card -->
                <div v-else
                  class="tool-card"
                  :class="tc.status"
                >
                <div class="tool-header" @click="tc._expanded = !tc._expanded">
                  <span class="tool-icon">🔧</span>
                  <span class="tool-name">{{ tc.name }}</span>
                  <template v-for="(seg, i) in getToolSummary(tc)" :key="i">
                    <span class="tool-seg" :class="'seg-' + seg.type">{{ seg.text }}</span>
                  </template>
                  <span class="tool-status" :class="tc.status">{{ tc.status }}</span>
                  <ChevronDown v-if="!tc._expanded" :size="14" class="tool-chevron" />
                  <ChevronUp v-else :size="14" class="tool-chevron" />
                </div>
                <div v-if="tc._expanded" class="tool-body">
                  <div class="tool-section">
                    <div class="tool-section-title">Arguments</div>
                    <pre class="tool-code">{{ JSON.stringify(tc.arguments, null, 2) }}</pre>
                  </div>
                  <div v-if="tc.result" class="tool-section">
                    <div class="tool-section-title">Result</div>
                    <pre class="tool-code result">{{ tc.result }}</pre>
                  </div>
                </div>
                </div>
              </template>
            </div>
            <!-- Message toolbar -->
            <div v-if="msg.role === 'user'" class="message-toolbar">
              <button class="toolbar-btn" title="Copy" @click="copyText(msg.content)">
                <Clipboard :size="13" />
              </button>
            </div>
            <div v-else-if="msg.role === 'assistant'" class="message-toolbar">
              <button class="toolbar-btn" title="Copy" @click="copyText(msg.content)">
                <Clipboard :size="13" />
              </button>
              <button class="toolbar-btn" title="Regenerate" @click="regenerate(msg)">
                <RefreshCw :size="13" />
              </button>
            </div>
          </div>
          <div v-if="isLoading && !hasPendingAssistant" class="message assistant pending">
            <div class="message-header">
              <AgentAvatar profession-id="assistant" :name="assistantName" size="sm" />
              <span class="role-badge assistant">{{ assistantName }}</span>
            </div>
            <div class="message-content">
              <span class="typing-dots">
                <span></span><span></span><span></span>
              </span>
            </div>
          </div>
          <div v-if="error" class="message error">
            <div class="message-content error">
              {{ error }}
            </div>
          </div>
        </div>
      </div>
      <!-- Current-session GateCard -->
      <GateCard
        v-if="needsApproval"
        title="Spec drafted. Review the proposed changes below."
        :changes="pendingSpecChanges"
        @approve="handleApprove"
        @reject="handleReject"
      />
      <div v-else class="chats-input-bar">
        <div class="input-inner">
          <div class="input-row">
            <div class="input-compose">
              <div class="input-backdrop" v-html="renderInputMentions(inputText)"></div>
              <textarea
                ref="textareaRef"
                v-model="inputText"
                class="chats-input"
                placeholder="Describe what you want to build... (Enter to send, @ for agent)"
                :disabled="isLoading"
                @input="handleInput"
                @keydown="handleKeydown"
                @keydown.enter.exact.prevent="sendMessage"
              />
            </div>
            <button
              class="send-btn"
              :disabled="!inputText.trim() || isLoading"
              @click="sendMessage"
            >
              <Send :size="16" />
            </button>
          </div>
          <MentionDropdown
            ref="mentionRef"
            :professions="professionOptions"
            :visible="mentionVisible"
            :filter="mentionFilter"
            :anchor-rect="mentionAnchor"
            @select="handleMentionSelect"
          />

        </div>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted, nextTick, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import {
  Send, ChevronDown, ChevronUp, Plus, PanelLeft,
  Check, X, Clipboard, RefreshCw, Search, Trash2, Pencil,
  Info, Copy, CopyCheck,
} from 'lucide-vue-next'
import { useForge } from '@/composables/useForge'
import { useGateInbox } from '@/composables/useGateInbox'
import { useProject } from '@/composables/useProject'
import { useAgentConfigs } from '@/composables/useAgentConfigs'
import { useRelay } from '@/composables/useRelay'
import { setEventCallbacks } from '@/composables/useEventRouter'
import StreamingRenderer from '@/components/StreamingRenderer.vue'
import MentionDropdown from '@/components/MentionDropdown.vue'
import AgentAvatar from '@/components/AgentAvatar.vue'
import GateCard from '@/components/GateCard.vue'
import SecretaryMessage from '@/components/SecretaryMessage.vue'
import ReportCard from '@/components/ReportCard.vue'
import QuestionnaireCard from '@/components/QuestionnaireCard.vue'
import type { ReportData } from '@/components/ReportCard.vue'
import type { Question } from '@/components/QuestionnaireCard.vue'

type AnswersMap = Record<string, string | string[]>

const { t } = useI18n()

const {
  session,
  messages,
  isLoading,
  error,
  sessionList,
  sessionId,
  needsApproval,
  pendingSpecChanges,
  resume,
  switchSession,
  clearSession,
  loadSessionList,
  sendMessage: forgeSendMessage,
  streamResponse,
  approveSpec,
  rejectSpec,
  renameSession,
  deleteSession,
  deleteAllSessions,
  errands,
  relayRuns,
  taskPlans,
} = useForge()

const { projectPath } = useProject()
const { currentSecretary, badgeCount: gateBadgeCount, resolveGate: resolveGateInbox, snoozeGate } = useGateInbox()
const { configs: agentConfigs, loadConfigs: loadAgentConfigs } = useAgentConfigs()
const { startRun } = useRelay()
const reportData = ref<ReportData | null>(null)

// @mention state
const mentionVisible = ref(false)
const mentionFilter = ref('')
const mentionAnchor = ref<DOMRect | null>(null)
const mentionRef = ref<InstanceType<typeof MentionDropdown>>()
const targetProfession = ref<string | null>(null)

const professionOptions = computed(() =>
  agentConfigs.value
    .filter(c => c.is_default)
    .map(c => ({ id: c.profession_id, name: c.name }))
)

const assistantName = computed(() =>
  agentConfigs.value.find(c => c.profession_id === 'assistant' && c.is_default)?.name ?? 'Assistant Agent'
)

function professionDisplayName(professionId: string): string {
  return agentConfigs.value.find(c => c.profession_id === professionId)?.name ?? professionId
}

function getHandoffTarget(tc: { arguments?: Record<string, unknown> }): string {
  const target = (tc.arguments?.target as string) || ''
  return professionDisplayName(target)
}

function handoffReason(tc: { arguments?: Record<string, unknown>; result?: string }): string {
  // Prefer arguments reason, fall back to parsed result JSON reason
  const argReason = (tc.arguments?.reason as string) || ''
  if (argReason.trim()) return argReason
  try {
    const result = JSON.parse(tc.result || '{}')
    return (result.reason as string) || 'Handed off to continue the conversation.'
  } catch {
    return tc.result || 'Handed off to continue the conversation.'
  }
}

// ─── Errand helpers ─────────────────────────────────────────────────────────

function getErrandByToolCallId(toolCallId: string) {
  return Object.values(errands.value).find((e) => e.tool_call_id === toolCallId) || null
}

function getErrandTask(tc: { arguments?: Record<string, unknown> }): string {
  return (tc.arguments?.task as string) || 'Research task'
}

function getErrandState(tc: { id: string }) {
  return getErrandByToolCallId(tc.id)
}

function getErrandContent(tc: { id: string }): string {
  return getErrandState(tc)?.content || ''
}

function getErrandStatus(tc: { id: string }) {
  return getErrandState(tc)
}

function getErrandToolCalls(tc: { id: string }) {
  return getErrandState(tc)?.tool_calls || []
}

// ─── Tool summary for inline header display ─────────────────────────────────

interface ToolSeg { type: string; text: string }

function getToolSummary(tc: { name: string; arguments?: Record<string, unknown> }): ToolSeg[] {
  const args = tc.arguments ?? {}
  const segs: ToolSeg[] = []

  const path = (args.path as string) || ''
  const slug = (args.slug as string) || ''
  const sectionId = (args.section_id as string) || ''
  const pattern = (args.pattern as string) || ''
  const query = (args.query as string) || ''
  const task = (args.task as string) || ''
  const command = (args.command as string) || ''
  const limit = args.limit as number | undefined
  const offset = args.offset as number | undefined

  if (path) {
    segs.push({ type: 'path', text: path })
    if (limit !== undefined || offset !== undefined) {
      segs.push({ type: 'loc', text: `:${limit ?? ''}:${offset ?? ''}` })
    }
  }
  if (slug) segs.push({ type: 'path', text: slug })
  if (sectionId) segs.push({ type: 'path', text: sectionId })
  if (pattern) {
    const s = pattern.length > 60 ? pattern.slice(0, 57) + '…' : pattern
    segs.push({ type: 'pattern', text: `"${s}"` })
  }
  if (query) {
    const s = query.length > 60 ? query.slice(0, 57) + '…' : query
    segs.push({ type: 'pattern', text: `"${s}"` })
  }
  if (task && !segs.length) {
    const s = task.length > 60 ? task.slice(0, 57) + '…' : task
    segs.push({ type: 'desc', text: s })
  }
  if (command && !segs.length) {
    const s = command.length > 80 ? command.slice(0, 77) + '…' : command
    segs.push({ type: 'desc', text: s })
  }

  return segs
}

// ─── Relay helpers ──────────────────────────────────────────────────────────

function getRelayStatus(tc: { arguments?: Record<string, unknown> }) {
  const runId = (tc.arguments?.run_id as string) || ''
  if (!runId) return null
  return relayRuns.value[runId] || null
}

function goToRelayRun(runId: string) {
  window.open(`/forge/relay?run=${encodeURIComponent(runId)}`, '_blank')
}

function openRelayView() {
  window.open('/forge/relay', '_blank')
}

/** Build a set of known agent names (lowercased) for mention detection */
const mentionNames = computed(() => {
  const names = new Map<string, string>() // lowercase name → display name
  for (const c of agentConfigs.value) {
    if (c.is_default) {
      names.set(c.name.toLowerCase(), c.name)
      names.set(c.profession_id.toLowerCase(), c.name)
    }
  }
  return names
})

/** Escape HTML, then wrap @mentions in styled spans */
function renderMentions(text: string): string {
  const escaped = text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
  return escaped.replace(/@(\w+)/g, (match, name) => {
    const displayName = mentionNames.value.get(name.toLowerCase())
    if (displayName) {
      return `<span class="inline-mention">@${displayName}</span>`
    }
    return match
  })
}

/** Same as renderMentions but for the input backdrop (adds trailing newline) */
function renderInputMentions(text: string): string {
  if (!text) return ''
  return renderMentions(text) + '\n'
}

function handleInput(e: Event) {
  const el = e.target as HTMLTextAreaElement
  const val = el.value
  const pos = el.selectionStart

  // Look backwards from cursor for a @ that starts a mention
  const textBeforeCursor = val.slice(0, pos)
  const atIdx = textBeforeCursor.lastIndexOf('@')
  if (atIdx >= 0) {
    // @ must be at start of text or preceded by whitespace
    const charBefore = atIdx > 0 ? val[atIdx - 1] : ''
    if (charBefore === '' || /\s/.test(charBefore)) {
      const afterAt = textBeforeCursor.slice(atIdx + 1)
      // Only show dropdown if no spaces after @ (still typing the name)
      if (!afterAt.includes(' ')) {
        mentionFilter.value = afterAt
        mentionAnchor.value = el.getBoundingClientRect()
        mentionVisible.value = true
        return
      }
    }
  }
  mentionVisible.value = false
}

function handleMentionSelect(id: string) {
  targetProfession.value = id
  const name = professionDisplayName(id)
  // Replace the @filter at cursor position with @DisplayName
  const val = inputText.value
  const ta = textareaRef.value as HTMLTextAreaElement | undefined
  const pos = ta?.selectionStart ?? val.length
  const textBeforeCursor = val.slice(0, pos)
  const atIdx = textBeforeCursor.lastIndexOf('@')
  if (atIdx >= 0) {
    const before = val.slice(0, atIdx)
    const after = val.slice(pos)
    inputText.value = `${before}@${name} ${after}`
  } else {
    inputText.value = `@${name} ${val}`
  }
  mentionVisible.value = false
  // Focus textarea after dropdown closes
  nextTick(() => {
    const ta = textareaRef.value as HTMLTextAreaElement | undefined
    ta?.focus()
  })
}

function handleGlobalKeydown(e: KeyboardEvent) {
  // Ctrl+Shift+N (or Cmd+Shift+N on macOS): Create new session
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'n') {
    e.preventDefault()
    clearSession(projectPath?.value ?? undefined)
    return
  }

  // Ctrl+Shift+S (or Cmd+Shift+S on macOS): Focus search input
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 's') {
    e.preventDefault()
    searchInputRef.value?.focus()
    return
  }
}

function handleKeydown(e: KeyboardEvent) {
  // Mention dropdown keyboard navigation
  if (!mentionVisible.value || !mentionRef.value?.hasItems()) return
  if (e.key === 'ArrowDown') {
    e.preventDefault()
    mentionRef.value.moveDown()
  } else if (e.key === 'ArrowUp') {
    e.preventDefault()
    mentionRef.value.moveUp()
  } else if (e.key === 'Enter' || e.key === 'Tab') {
    e.preventDefault()
    const id = mentionRef.value.currentId()
    if (id) handleMentionSelect(id)
  } else if (e.key === 'Escape') {
    mentionVisible.value = false
  }
}

const expandedDiffs = ref<Set<string>>(new Set())
const editedSpecs = ref<Record<string, string>>({})
const chatSearch = ref('')
const thinkingMode = ref(false)
const searchInputRef = ref<HTMLInputElement>()
const allErrandsExpanded = ref(false)

function toggleAllErrands() {
  allErrandsExpanded.value = !allErrandsExpanded.value
  // Update all errand cards
  for (const key in errands.value) {
    const e = errands.value[key]
    if (e) {
      ;(e as any)._expanded = allErrandsExpanded.value
    }
  }
}

const projectName = computed(() => {
  const path = session.value?.project_path
  if (!path || path === '.') return null
  // Extract last dir name from path
  const parts = path.replace(/\\/g, '/').split('/').filter(Boolean)
  return parts.length > 0 ? parts[parts.length - 1] : null
})
const textareaRef = ref<HTMLTextAreaElement>()

const filteredMessages = computed(() => {
  const q = chatSearch.value.trim().toLowerCase()
  if (!q) return messages.value
  return messages.value.filter((m) =>
    m.content.toLowerCase().includes(q) ||
    m.role.toLowerCase().includes(q)
  )
})
function toggleDiff(sectionId: string) {
  if (expandedDiffs.value.has(sectionId)) {
    expandedDiffs.value.delete(sectionId)
  } else {
    expandedDiffs.value.add(sectionId)
  }
}

watch(pendingSpecChanges, (changes) => {
  for (const change of changes) {
    if (!(change.section_id in editedSpecs.value)) {
      editedSpecs.value[change.section_id] = change.new_content
    }
  }
}, { immediate: true })

const CHAT_SIDEBAR_KEY = 'autoforge-chat-sidebar-collapsed'

const inputText = ref('')
const chatRef = ref<HTMLDivElement>()
const sidebarCollapsed = ref(localStorage.getItem(CHAT_SIDEBAR_KEY) === 'true')
const showSessionInfo = ref(false)
const copiedChatId = ref(false)

const sessionTokenCost = computed(() => {
  let total = 0
  for (const key in errands.value) {
    total += errands.value[key].token_usage || 0
  }
  for (const key in relayRuns.value) {
    total += relayRuns.value[key].tokens_used || 0
  }
  return total > 0 ? `${total.toLocaleString()} tok` : '—'
})

function copyChatId() {
  if (!sessionId.value) return
  navigator.clipboard.writeText(sessionId.value).then(() => {
    copiedChatId.value = true
    setTimeout(() => { copiedChatId.value = false }, 2000)
  })
}

watch(sidebarCollapsed, (v) => {
  localStorage.setItem(CHAT_SIDEBAR_KEY, String(v))
})

const hasPendingAssistant = computed(() => {
  return messages.value.some((m) => m.role === 'assistant' && m.content === '' && !m.tool_calls?.length)
})

const lastAssistantMessage = computed(() => {
  for (let i = messages.value.length - 1; i >= 0; i--) {
    if (messages.value[i].role === 'assistant') {
      return messages.value[i]
    }
  }
  return null
})

function isStreamingMessage(msg: typeof messages.value[number]): boolean {
  return isLoading.value && msg === lastAssistantMessage.value
}

/** Parse questionnaire JSON from a message, returning questions + stripped content */
function questionnaireFor(msg: typeof messages.value[number]): { questions: Question[]; strippedContent: string } | undefined {
  if (msg.role !== 'assistant') return undefined

  // 1. Try structured JSON block first
  const blockRegex = /```json\s*\n([\s\S]*?)\n\s*```/g
  let match: RegExpExecArray | null
  while ((match = blockRegex.exec(msg.content)) !== null) {
    try {
      const json = JSON.parse(match[1].trim())
      if (json.type === 'questionnaire' && Array.isArray(json.questions) && json.questions.length > 0) {
        const stripped = msg.content.replace(match[0], '').trim()
        return { questions: json.questions, strippedContent: stripped }
      }
    } catch { /* ignore invalid JSON */ }
  }

  // 2. Fallback: detect free-text questions with optional sub-bullet options
  const lines = msg.content.split('\n')
  const questions: Question[] = []
  let consumedLines: number[] = []

  function isBullet(line: string): boolean {
    return /^\s*(?:[-*•])\s+/.test(line)
  }

  function isNumbered(line: string): boolean {
    return /^\s*\d+\.\s+/.test(line)
  }

  function stripNumbering(text: string): string {
    return text.replace(/^\s*\d+\.\s+/, '').trim()
  }

  function stripMarkdown(text: string): string {
    return text.replace(/\*\*/g, '').trim()
  }

  // Pass 1: scan for parent questions (numbered or standalone lines ending with ?)
  // followed by child bullet options
  let i = 0
  while (i < lines.length) {
    const line = lines[i].trim()

    if (!line || line.startsWith('```') || consumedLines.includes(i)) {
      i++
      continue
    }

    // A parent question:
    // - Numbered items: contains ? anywhere (lenient — numbered lists are usually questions)
    // - Non-numbered: must end with ?
    // - Must not be a bullet itself
    // - Must have reasonable length
    const isNumberedQuestion = isNumbered(line) && line.includes('?') && line.length > 10
    const isStandaloneQuestion = !isNumbered(line) && !isBullet(line) && line.endsWith('?') && line.length > 15
    const isParentCandidate = isNumberedQuestion || isStandaloneQuestion

    if (isParentCandidate) {
      // Look ahead for child bullet options
      const childOptions: string[] = []
      let j = i + 1
      while (j < lines.length) {
        const nextLine = lines[j]
        if (isBullet(nextLine)) {
          const optText = stripMarkdown(nextLine.replace(/^\s*(?:[-*•])\s+/, '').trim())
          if (optText) childOptions.push(optText)
          j++
        } else if (nextLine.trim() === '') {
          j++
        } else {
          break
        }
      }

      const parentText = stripMarkdown(stripNumbering(line))

      if (childOptions.length >= 2) {
        questions.push({
          id: `q${questions.length + 1}`,
          text: parentText,
          type: 'multiple',
          options: childOptions,
          otherLabel: 'Other:',
          otherPlaceholder: 'Type additional details...',
        })
        consumedLines.push(i)
        for (let k = i + 1; k < j; k++) consumedLines.push(k)
        i = j
        continue
      } else if (childOptions.length === 1) {
        questions.push({
          id: `q${questions.length + 1}`,
          text: parentText,
          type: 'single',
          options: [childOptions[0]],
          otherLabel: 'Other:',
          otherPlaceholder: 'Type additional details...',
        })
        consumedLines.push(i)
        for (let k = i + 1; k < j; k++) consumedLines.push(k)
        i = j
        continue
      } else {
        questions.push({ id: `q${questions.length + 1}`, text: parentText, type: 'text', placeholder: 'Type your answer...' })
        consumedLines.push(i)
        i++
        continue
      }
    }

    i++
  }

  // Pass 2: detect inline options after colon (comma or "or" separated)
  for (let i = 0; i < lines.length; i++) {
    if (consumedLines.includes(i)) continue
    const line = lines[i].trim()
    if (!line.endsWith('?') || line.startsWith('```') || line.length <= 15) continue

    // Pattern: "Label: opt1, opt2, or opt3?" or "Label — opt1 / opt2 / opt3?"
    const colonMatch = line.match(/^(.+?)[:：\u2014\u2013\u2015-]\s*(.+)\?\s*$/)
    if (colonMatch) {
      const label = stripMarkdown(colonMatch[1]).trim()
      const optionsText = colonMatch[2]
      // Split by comma, slash, or " or "
      const opts = optionsText
        .split(/[,，、/\/]|\s+or\s+/)
        .map(s => stripMarkdown(s).trim())
        .filter(s => s.length > 0 && s.toLowerCase() !== 'etc')
      if (opts.length >= 2) {
        questions.push({
          id: `q${questions.length + 1}`,
          text: label,
          type: 'single',
          options: opts,
          otherLabel: 'Other:',
          otherPlaceholder: 'Type additional details...',
        })
        consumedLines.push(i)
      }
    }
  }

  // Pass 3: detect markdown tables that act as questionnaires (rows with ? placeholders)
  if (questions.length === 0) {
    // Scan for markdown table blocks
    let tableStart = -1
    let tableEnd = -1
    for (let i = 0; i < lines.length; i++) {
      if (lines[i].trim().startsWith('|') && lines[i].includes('|', lines[i].indexOf('|') + 1)) {
        if (tableStart === -1) tableStart = i
        tableEnd = i
      } else if (tableStart !== -1 && lines[i].trim() === '') {
        // empty line ends table
        break
      }
    }

    if (tableStart !== -1 && tableEnd > tableStart + 1) {
      const tableLines = lines.slice(tableStart, tableEnd + 1)
      // Skip separator line (contains ---)
      const dataRows = tableLines.filter(l => !l.includes('---') && !l.includes(':--'))
      if (dataRows.length >= 2) {
        const headerCells = dataRows[0].split('|').map(c => c.trim()).filter(Boolean)
        const hasPriorityHeader = headerCells.some(h => /priority|priority/i.test(h))
        const rows = dataRows.slice(1)
        const hasPlaceholders = rows.some(r => r.includes('?') || r.includes('???'))

        if (hasPriorityHeader || hasPlaceholders) {
          // Try to extract priority options from preceding text
          const precedingText = lines.slice(0, tableStart).join('\n')
          const p0Match = precedingText.match(/P0\s*\(([^)]+)\)/)
          const p1Match = precedingText.match(/P1\s*\(([^)]+)\)/)
          const p2Match = precedingText.match(/P2\s*\(([^)]+)\)/)
          let priorityOptions = ['P0 (Critical)', 'P1 (Important)', 'P2 (Nice-to-have)']
          if (p0Match && p1Match && p2Match) {
            priorityOptions = [`P0 (${p0Match[1]})`, `P1 (${p1Match[1]})`, `P2 (${p2Match[1]})`]
          }

          for (const row of rows) {
            const cells = row.split('|').map(c => c.trim()).filter(Boolean)
            if (cells.length >= 2) {
              const featureName = cells[0].replace(/\*\*/g, '').trim()
              if (featureName && !featureName.toLowerCase().includes('other')) {
                questions.push({
                  id: `q${questions.length + 1}`,
                  text: `Priority for "${featureName}"`,
                  type: 'single',
                  options: priorityOptions,
                })
              }
            }
          }
          consumedLines = Array.from({ length: tableEnd - tableStart + 1 }, (_, i) => tableStart + i)
        }
      }
    }
  }

  if (questions.length >= 2) {
    const remaining = lines.filter((_, idx) => !consumedLines.includes(idx)).join('\n').trim()
    return { questions, strippedContent: remaining }
  }

  return undefined
}

/** Handle questionnaire submission: send compact numbered answers */
async function onQuestionnaireSubmit(answers: Record<string, string | string[]>, msg: typeof messages.value[number]) {
  const q = questionnaireFor(msg)
  if (!q) return
  const parts: string[] = []
  for (let idx = 0; idx < q.questions.length; idx++) {
    const question = q.questions[idx]
    const answer = answers[question.id]
    const other = answers[`${question.id}__other`] as string | undefined
    const qNum = `Q${idx + 1}`
    if (Array.isArray(answer) && answer.length > 0) {
      const ans = answer.join(', ') + (other ? `, Other: ${other}` : '')
      parts.push(`${qNum}: ${ans}`)
    } else if (answer && (answer as string).trim() !== '') {
      parts.push(`${qNum}: ${answer}${other ? `, Other: ${other}` : ''}`)
    } else if (other) {
      parts.push(`${qNum}: Other: ${other}`)
    }
  }
  if (parts.length === 0) return
  const text = parts.join('; ')
  inputText.value = text
  await sendMessage()
}

function formatTime(ts: number): string {
  return new Date(ts).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })
}

async function scrollToBottom() {
  await nextTick()
  if (chatRef.value) {
    chatRef.value.scrollTop = chatRef.value.scrollHeight
  }
}

watch(messages, scrollToBottom, { deep: true })

/** Resolve an @mention word to a profession_id (handles both names and ids) */
function resolveMention(word: string): string | undefined {
  const lower = word.toLowerCase()
  // Try exact profession_id match first
  if (agentConfigs.value.some(c => c.profession_id === lower)) return lower
  // Try matching against agent display names (case-insensitive)
  const match = agentConfigs.value.find(c => c.name.toLowerCase() === lower)
  return match?.profession_id
}

async function sendMessage() {
  const text = inputText.value.trim()
  if (!text) return
  inputText.value = ''
  mentionVisible.value = false

  // ─── Quick Relay shortcut: /relay <goal> ──────────────────────────────
  if (text.startsWith('/relay ')) {
    const goal = text.slice('/relay '.length).trim()
    if (!goal) return
    const runId = await startRun({
      flow_id: 'auto-discovery',
      task: goal,
      steps: [
        { id: 'discover', profession_id: 'advisor' },
        { id: 'design', profession_id: 'architect' },
        { id: 'plan', profession_id: 'planner' },
        { id: 'draft-tests', profession_id: 'tester' },
        { id: 'code', profession_id: 'coder' },
        { id: 'run-tests', profession_id: 'tester' },
        { id: 'review', profession_id: 'reviewer' },
        { id: 'report', profession_id: 'documenter' },
      ],
    })
    if (runId) {
      messages.value.push({
        id: `relay-${runId}`,
        role: 'assistant',
        content: `🚀 **Relay Run 已开始自动执行**\n\n**目标**: ${goal}\n**Run ID**: \`${runId}\`\n**Flow**: auto-discovery\n\nAdvisor 正在自动分析 Goals 和 Subgoals，随后将无缝传递给 Architect → Planner → Coder → Tester → Reviewer → Documenter。你可以在 [Relay 视图](/forge/relay?run=${encodeURIComponent(runId)}) 中实时查看进度。`,
        timestamp: Date.now(),
        profession_id: 'assistant',
      })
    }
    return
  }

  // ─── Quick Spec1 shortcut: /spec1 <goal> ──────────────────────────────
  // Runs only the Advisor step to test goal-discovery before full pipeline.
  if (text.startsWith('/spec1 ')) {
    const goal = text.slice('/spec1 '.length).trim()
    if (!goal) return
    const runId = await startRun({
      flow_id: 'goal-discovery',
      task: goal,
      steps: [
        { id: 'discover', profession_id: 'advisor' },
      ],
    })
    if (runId) {
      messages.value.push({
        id: `spec1-${runId}`,
        role: 'assistant',
        content: `🎯 **Goal Discovery Run 已开始**\n\n**目标**: ${goal}\n**Run ID**: \`${runId}\`\n**Flow**: goal-discovery\n\nAdvisor 正在自动分析并尝试写出 Goals。此 Run 只执行 Advisor 一步，成功写出 Goal 即结束。你可以在 [Relay 视图](/forge/relay?run=${encodeURIComponent(runId)}) 中实时查看进度。`,
        timestamp: Date.now(),
        profession_id: 'assistant',
      })
    }
    return
  }

  // Extract profession_id from the first @mention in text for routing,
  // but keep the full text (including @mention) as the message content
  let professionId: string | undefined = targetProfession.value ?? undefined
  targetProfession.value = null

  const mentionMatch = text.match(/(?:^|\s)@(\w+)/)
  if (mentionMatch) {
    const resolved = resolveMention(mentionMatch[1])
    if (resolved) professionId = resolved
  }

  await forgeSendMessage(text, professionId)
}

async function handleApprove(editedSpecs?: Record<string, string>) {
  await approveSpec(editedSpecs)
  await streamResponse()
}

async function handleReject() {
  await rejectSpec()
}

// ─── Secretary handlers ─────────────────────────────────────────────────────

async function onSecretaryApprove(gateId: string) {
  resolveGateInbox(gateId, 'approved')
}

async function onSecretaryReject(gateId: string) {
  resolveGateInbox(gateId, 'rejected')
}

function onSecretarySnooze(gateId: string) {
  snoozeGate(gateId)
}

function onReviewInSpecs(sectionId: string) {
  // Emit or navigate to specs view with section active
  alert(`Navigate to specs section: ${sectionId}`)
}

// ─── Report handlers ────────────────────────────────────────────────────────

function onViewFullReport() {
  alert('View full report in specs')
}

function onDownloadReport() {
  if (!reportData.value) return
  const blob = new Blob([`# Report ${reportData.value.runId}\n\n`], { type: 'text/markdown' })
  const url = URL.createObjectURL(blob)
  const a = document.createElement('a')
  a.href = url
  a.download = `report-${reportData.value.runId}.md`
  a.click()
  URL.revokeObjectURL(url)
}

function onOpenChangedFiles() {
  alert('Open changed files')
}

const editingSessionId = ref<string | null>(null)
const editingName = ref('')

function startRename(s: { id: string; name?: string; preview: string }) {
  editingSessionId.value = s.id
  editingName.value = s.name || s.preview
  nextTick(() => {
    const el = document.querySelector<HTMLInputElement>(`[data-rename-input="${s.id}"]`)
    el?.focus()
    el?.select()
  })
}

async function commitRename() {
  const sid = editingSessionId.value
  if (!sid) return
  const name = editingName.value.trim()
  if (name) {
    await renameSession(sid, name)
  }
  editingSessionId.value = null
  editingName.value = ''
}

function cancelRename() {
  editingSessionId.value = null
  editingName.value = ''
}

async function confirmDelete(sid: string) {
  const ok = confirm('Delete this session? All messages and memory will be lost.')
  if (!ok) return
  await deleteSession(sid)
}

async function confirmDeleteAll() {
  const ok = confirm(t('chat.confirmDeleteAll'))
  if (!ok) return
  await deleteAllSessions()
}

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text)
  } catch {
    // fallback
    const ta = document.createElement('textarea')
    ta.value = text
    document.body.appendChild(ta)
    ta.select()
    document.execCommand('copy')
    document.body.removeChild(ta)
  }
}

function regenerate(_msg: typeof messages.value[number]) {
  // TODO: wire up to backend regenerate endpoint
  alert('Regenerate: not yet implemented')
}

onMounted(async () => {
  window.addEventListener('keydown', handleGlobalKeydown)

  // Wire report callback from event router
  setEventCallbacks({
    onReport: (payload) => {
      reportData.value = {
        runId: (payload.run_id as string) || 'unknown',
        goalsMet: (payload.goals_met as string) || '—',
        testsPass: (payload.tests_pass as string) || '—',
        driftDetected: (payload.drift_detected as string) || 'None',
        cost: (payload.cost as string) || '—',
        confidence: (payload.confidence as 'High' | 'Medium' | 'Low') || 'Medium',
        deliverables: (payload.deliverables as string[]) || [],
      }
    },
  })
  if (!session.value) {
    await resume(projectPath?.value ?? undefined)
  }
  await loadSessionList()
  await loadAgentConfigs()
})

onUnmounted(() => {
  window.removeEventListener('keydown', handleGlobalKeydown)
})
</script>

<style scoped>
.chats-view {
  display: flex;
  flex-direction: row;
  height: 100%;
  overflow: hidden;
}

/* ─── Session Sidebar ─────────────────────────────────────────────────────── */

.session-sidebar {
  width: 220px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  background: transparent;
  border-right: 1px solid var(--af-border);
  transition: width 0.2s ease, margin-left 0.2s ease;
}

.session-sidebar.collapsed {
  width: 0;
  margin-left: -1px;
  overflow: hidden;
}

.sidebar-header {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.75rem 1rem;
  flex-shrink: 0;
  height: 48px;
  border-bottom: 1px solid var(--af-border);
}

.sidebar-title {
  font-size: 0.95rem;
  font-weight: 500;
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  flex: 1;
  line-height: 1;
}

.sidebar-new-btn,
.sidebar-delete-all-btn,
.sidebar-collapse-btn,
.sidebar-toggle-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 26px;
  height: 26px;
  background: transparent;
  border: none;
  border-radius: 5px;
  color: var(--af-muted);
  cursor: pointer;
  transition: all 0.15s;
}

.sidebar-new-btn:hover,
.sidebar-delete-all-btn:hover,
.sidebar-collapse-btn:hover,
.sidebar-toggle-btn:hover {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-fg);
}

.sidebar-delete-all-btn {
  color: hsl(0 60% 50%);
}
.sidebar-delete-all-btn:hover {
  color: hsl(0 70% 45%);
  background: hsl(0 60% 50% / 0.1);
}

.session-list {
  flex: 1;
  overflow-y: auto;
  padding: 0 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.15rem;
}

.session-item {
  padding: 0.5rem 0.6rem;
  border-radius: 6px;
  cursor: pointer;
  transition: all 0.15s;
}

.session-item:hover {
  background: hsl(var(--muted-foreground) / 0.05);
}

.session-item.active {
  background: hsl(var(--primary) / 0.06);
  border-left: 2px solid var(--af-primary);
  margin-left: -2px;
}

.session-preview {
  font-size: 0.88rem;
  color: var(--af-fg);
  line-height: 1.4;
  overflow: hidden;
  text-overflow: ellipsis;
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
}

.session-meta {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  margin-top: 0.2rem;
}

.session-rename-btn,
.session-delete-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--af-muted);
  cursor: pointer;
  opacity: 0;
  transition: all 0.15s;
  flex-shrink: 0;
}

.session-rename-btn {
  margin-left: auto;
}

.session-item:hover .session-rename-btn,
.session-item:hover .session-delete-btn {
  opacity: 1;
}

.session-rename-btn:hover {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-fg);
}

.session-delete-btn:hover {
  background: hsl(var(--af-error) / 0.1);
  color: hsl(var(--af-error));
}

.session-rename-input {
  width: 100%;
  font-size: 0.88rem;
  color: var(--af-fg);
  background: hsl(var(--muted-foreground) / 0.06);
  border: 1px solid hsl(var(--primary) / 0.35);
  border-radius: 4px;
  padding: 0.2rem 0.4rem;
  outline: none;
  font-family: inherit;
  line-height: 1.4;
}

.session-count {
  font-size: 0.73rem;
  color: var(--af-muted);
}

.session-empty {
  font-size: 0.88rem;
  color: var(--af-muted);
  text-align: center;
  padding: 1rem 0;
}

/* ─── Chat Area ───────────────────────────────────────────────────────────── */

.chats-body {
  display: flex;
  flex-direction: column;
  flex: 1;
  min-width: 0;
  min-height: 0;
}

.chats-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  flex-shrink: 0;
  height: 48px;
  position: relative;
  border-bottom: 1px solid var(--af-border);
}

.header-title-row {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  flex-shrink: 0;
  width: 80px;
}

.chats-header h2 {
  font-size: 0.83rem;
  font-weight: 500;
  color: var(--af-muted);
  text-transform: uppercase;
  letter-spacing: 0.04em;
  line-height: 1;
}

.header-center {
  display: flex;
  align-items: center;
  gap: 0.75rem;
  justify-content: center;
  position: absolute;
  left: 50%;
  transform: translateX(-50%);
}

.header-project {
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--af-primary);
  line-height: 1;
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

.header-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.session-info-wrapper {
  position: relative;
}

.session-info-btn {
  background: transparent;
  border: none;
  color: var(--af-muted);
  cursor: pointer;
  padding: 0.25rem;
  border-radius: 0.25rem;
  display: flex;
  align-items: center;
  justify-content: center;
  transition: color 0.15s, background 0.15s;
}

.session-info-btn:hover {
  color: var(--af-text);
  background: var(--af-surface);
}

.session-info-tooltip {
  position: absolute;
  top: calc(100% + 0.5rem);
  right: 0;
  min-width: 280px;
  background: var(--af-bg);
  border: 1px solid var(--af-border);
  border-radius: 0.5rem;
  padding: 0.75rem;
  box-shadow: 0 4px 12px rgba(0,0,0,0.15);
  z-index: 100;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.session-info-row {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.session-info-label {
  font-size: 0.78rem;
  color: var(--af-muted);
  min-width: 70px;
  flex-shrink: 0;
}

.session-info-value {
  font-size: 0.85rem;
  color: var(--af-text);
  flex: 1;
}

.session-info-id {
  font-family: monospace;
  font-size: 0.78rem;
  word-break: break-all;
}

.session-info-copy {
  background: transparent;
  border: none;
  color: var(--af-muted);
  cursor: pointer;
  padding: 0.15rem;
  border-radius: 0.25rem;
  display: flex;
  align-items: center;
  transition: color 0.15s;
}

.session-info-copy:hover {
  color: var(--af-primary);
}

.chat-canvas {
  flex: 1;
  overflow-y: auto;
  padding: 0.75rem 1rem;
  display: flex;
  flex-direction: column;
}

.chat-inner {
  max-width: 960px;
  width: 100%;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: 1.5rem;
}

.message {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.message.user {
  align-self: flex-end;
  max-width: 85%;
}

.message.assistant,
.message.system {
  align-self: flex-start;
  max-width: 100%;
}

.message.error {
  align-self: center;
  max-width: 100%;
}

.message-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0 0.25rem;
}

.role-badge {
  font-size: 0.9rem;
  font-weight: 500;
  color: var(--af-muted);
}

.role-badge.user {
  color: var(--af-primary);
  font-weight: 600;
}

.user-text {
  white-space: pre-wrap;
  word-break: break-word;
}

.user-text :deep(.inline-mention) {
  display: inline;
  font-weight: 600;
  color: var(--af-primary);
  background: hsl(var(--primary) / 0.1);
  padding: 0 4px;
  border-radius: 4px;
  cursor: default;
}

.msg-time {
  font-size: 0.73rem;
  color: var(--af-muted);
}

.message-content {
  font-size: 1rem;
  line-height: 1.6;
  color: var(--af-fg);
  white-space: pre-wrap;
  word-break: break-word;
  padding: 0.25rem 0;
}

/* Override markstream-vue heading sizes to match body text scale */
.message-content :deep(.markstream-vue) {
  --ms-text-body: 1.0rem;
}

.message-content :deep(h1),
.message-content :deep(h2),
.message-content :deep(h3),
.message-content :deep(h4) {
  font-size: 1.03rem;
  font-weight: 600;
  margin: 0.75rem 0 0.35rem;
  line-height: 1.4;
}

.message-content :deep(p) {
  margin: 0.35rem 0;
}

.message-content :deep(ul),
.message-content :deep(ol) {
  margin: 0.35rem 0;
  padding-left: 1.25rem;
}

.message-content :deep(li) {
  margin: 0.15rem 0;
}

.message-content :deep(pre) {
  margin: 0.5rem 0;
}

.message-content :deep(hr) {
  margin: 0.75rem 0;
  border: none;
  border-top: 1px solid var(--af-border);
}

.message-content.has-border {
  border-top: 1px solid var(--af-border);
  border-bottom: 1px solid var(--af-border);
  padding: 0.5rem 0;
  margin: 0.25rem 0;
}

.message-content.error {
  color: hsl(var(--af-error));
  font-size: 0.93rem;
}

.message.user .message-content {
  background: hsl(var(--primary) / 0.06);
  border-radius: 12px;
  padding: 0.6rem 0.9rem;
  max-width: 100%;
  font-size: 0.98rem;
}

.message.system .message-content {
  font-size: 0.93rem;
}

/* ─── Thinking Block ──────────────────────────────────────────────────────── */

.thinking-block {
  margin-top: 0.3rem;
  margin-bottom: 0.3rem;
}

.thinking-block details {
  border: 1px solid hsl(var(--muted-foreground) / 0.12);
  border-radius: 8px;
  background: hsl(var(--muted-foreground) / 0.03);
  overflow: hidden;
}

.thinking-block details[open] {
  background: hsl(var(--muted-foreground) / 0.05);
}

.thinking-block summary {
  display: flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.35rem 0.6rem;
  font-size: 0.78rem;
  color: var(--af-muted);
  cursor: pointer;
  user-select: none;
  list-style: none;
}

.thinking-block summary::-webkit-details-marker {
  display: none;
}

.thinking-icon {
  font-size: 0.85rem;
  line-height: 1;
}

.thinking-label {
  font-weight: 500;
}

.thinking-pulse {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: hsl(var(--primary) / 0.7);
  animation: thinkingPulse 1.5s infinite;
}

@keyframes thinkingPulse {
  0% { opacity: 1; transform: scale(1); }
  50% { opacity: 0.3; transform: scale(1.4); }
  100% { opacity: 1; transform: scale(1); }
}

.thinking-content {
  padding: 0.5rem 0.75rem;
  margin: 0;
  font-size: 0.82rem;
  line-height: 1.5;
  color: var(--af-muted);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 300px;
  overflow-y: auto;
  background: transparent;
  border: none;
}

.system-welcome {
  display: flex;
  align-items: flex-start;
  gap: 0.5rem;
  color: var(--af-muted);
  line-height: 1.5;
}

.welcome-icon {
  font-size: 1.18rem;
  flex-shrink: 0;
}

/* ─── Message Toolbar ─────────────────────────────────────────────────────── */

.message-toolbar {
  display: flex;
  align-items: center;
  gap: 0.15rem;
  padding: 0.1rem 0.25rem;
  opacity: 0;
  transition: opacity 0.15s;
}

.message:hover .message-toolbar {
  opacity: 1;
}

.toolbar-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  background: transparent;
  border: none;
  border-radius: 4px;
  color: var(--af-muted);
  cursor: pointer;
  transition: all 0.15s;
}

.toolbar-btn:hover {
  background: hsl(var(--muted-foreground) / 0.08);
  color: var(--af-fg);
}

/* ─── Tool Cards ──────────────────────────────────────────────────────────── */

.tool-calls {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  margin-top: 0.15rem;
}

/* ─── Handoff Card ─────────────────────────────────────────────────────────── */

.handoff-card {
  background: linear-gradient(135deg, hsl(var(--primary) / 0.06), hsl(var(--primary) / 0.02));
  border: 1px solid hsl(var(--primary) / 0.2);
  border-radius: 10px;
  padding: 0.6rem 0.8rem;
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem 0.8rem;
}

.handoff-flow {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-weight: 600;
  font-size: 0.98rem;
}

.handoff-agent {
  color: var(--af-fg);
}

.handoff-agent.target {
  color: hsl(var(--primary));
}

.handoff-arrow {
  color: var(--af-muted);
  font-size: 1.18rem;
}

.handoff-reason {
  color: var(--af-muted);
  font-size: 0.85rem;
  flex-basis: 100%;
}

.handoff-badge {
  background: hsl(var(--primary) / 0.12);
  color: hsl(var(--primary));
  padding: 0.1rem 0.5rem;
  border-radius: 999px;
  font-size: 0.75rem;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.03em;
}

.tool-card {
  background: transparent;
  border: 1px solid var(--af-border);
  border-radius: 8px;
  overflow: hidden;
}

.tool-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.6rem;
  cursor: pointer;
  user-select: none;
}

.tool-header:hover {
  background: hsl(var(--muted-foreground) / 0.03);
}

.tool-icon {
  font-size: 0.93rem;
}

.tool-name {
  font-size: 0.83rem;
  font-weight: 500;
  color: var(--af-fg);
  flex: 1;
}

.tool-status {
  font-size: 0.73rem;
  font-weight: 500;
  color: var(--af-muted);
}

.tool-status.pending { color: hsl(var(--af-warning)); }
.tool-status.running { color: hsl(var(--af-info)); }
.tool-status.success { color: hsl(var(--af-success)); }
.tool-status.error { color: hsl(var(--af-error)); }

/* Inline tool summary segments */
.tool-seg {
  font-size: 0.78rem;
  font-weight: 500;
  font-family: 'Geist Mono', 'Fira Code', monospace;
}
.tool-seg.seg-path { color: hsl(var(--af-info)); }
.tool-seg.seg-loc { color: var(--af-muted); }
.tool-seg.seg-pattern { color: hsl(var(--af-warning)); }
.tool-seg.seg-desc { color: hsl(var(--af-chats)); }

.tool-chevron {
  color: var(--af-muted);
}

.tool-body {
  border-top: 1px solid var(--af-border);
  padding: 0.5rem 0.6rem;
}

.tool-section {
  margin-bottom: 0.4rem;
}

.tool-section:last-child {
  margin-bottom: 0;
}

.tool-section-title {
  font-size: 0.73rem;
  font-weight: 500;
  text-transform: uppercase;
  color: var(--af-muted);
  margin-bottom: 0.2rem;
  letter-spacing: 0.02em;
}

.tool-code {
  font-size: 0.83rem;
  color: var(--af-muted);
  background: hsl(var(--muted-foreground) / 0.04);
  padding: 0.35rem 0.5rem;
  border-radius: 4px;
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.tool-code.result {
  color: hsl(var(--af-success));
}

/* Shell tool card */
.shell-card {
  background: hsl(var(--muted-foreground) / 0.03);
  border: 1px solid var(--af-border);
  border-radius: 6px;
  padding: 0.35rem 0.6rem;
}

.shell-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.shell-icon {
  color: var(--af-success);
  font-weight: 700;
  font-size: 0.88rem;
}

.shell-cmd {
  font-size: 0.86rem;
  color: var(--af-fg);
  flex: 1;
  font-family: monospace;
}

.shell-status {
  font-size: 0.73rem;
  font-weight: 500;
  color: var(--af-muted);
}

.shell-status.pending { color: hsl(var(--af-warning)); }
.shell-status.running { color: hsl(var(--af-info)); }
.shell-status.success { color: hsl(var(--af-success)); }
.shell-status.error { color: hsl(var(--af-error)); }

.shell-output {
  margin: 0.4rem 0 0;
  padding: 0.35rem 0.5rem;
  font-size: 0.81rem;
  color: var(--af-muted);
  background: hsl(var(--muted-foreground) / 0.04);
  border-radius: 4px;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 200px;
  overflow-y: auto;
}

.shell-toggle {
  background: none;
  border: none;
  color: var(--af-primary);
  font-size: 0.73rem;
  cursor: pointer;
  padding: 0.15rem 0;
  margin-top: 0.2rem;
}

/* ─── Errand Card ─────────────────────────────────────────────────────────── */

.errand-card {
  background: hsl(var(--muted-foreground) / 0.03);
  border: 1px solid var(--af-border);
  border-radius: 6px;
  overflow: hidden;
}

.errand-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
  padding: 0.35rem 0.6rem;
  cursor: pointer;
  user-select: none;
}

.errand-icon {
  font-size: 0.9rem;
}

.errand-name {
  font-size: 0.84rem;
  font-weight: 500;
  color: var(--af-fg);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.errand-status {
  font-size: 0.73rem;
  font-weight: 500;
  text-transform: capitalize;
}

.errand-status.running { color: hsl(var(--af-info)); }
.errand-status.completed { color: hsl(var(--af-success)); }
.errand-status.failed { color: hsl(var(--af-error)); }
.errand-status.truncated { color: hsl(var(--af-warning)); }

.errand-cost {
  font-size: 0.7rem;
  color: var(--af-muted);
  background: hsl(var(--muted-foreground) / 0.08);
  padding: 0.1rem 0.35rem;
  border-radius: 4px;
}

.errand-body {
  padding: 0.35rem 0.6rem 0.5rem;
  border-top: 1px solid var(--af-border);
}

.errand-task {
  font-size: 0.8rem;
  color: var(--af-muted);
  margin-bottom: 0.4rem;
  font-style: italic;
}

.errand-content {
  font-size: 0.84rem;
  color: var(--af-fg);
  line-height: 1.45;
  margin-bottom: 0.4rem;
}

.errand-tool-calls {
  display: flex;
  flex-direction: column;
  gap: 0.3rem;
  margin-bottom: 0.4rem;
}

.errand-sub-tool {
  background: hsl(var(--muted-foreground) / 0.05);
  border-radius: 4px;
  padding: 0.25rem 0.4rem;
}

.errand-sub-tool-header {
  display: flex;
  align-items: center;
  gap: 0.3rem;
}

.errand-sub-tool-name {
  font-size: 0.78rem;
  font-weight: 500;
  color: var(--af-fg);
}

.errand-sub-tool-status {
  font-size: 0.7rem;
  font-weight: 500;
}

.errand-sub-tool-status.running { color: hsl(var(--af-info)); }
.errand-sub-tool-status.success { color: hsl(var(--af-success)); }
.errand-sub-tool-status.error { color: hsl(var(--af-error)); }

.errand-sub-tool-result {
  margin: 0.2rem 0 0;
  padding: 0.25rem 0.35rem;
  font-size: 0.78rem;
  color: var(--af-muted);
  background: hsl(var(--muted-foreground) / 0.04);
  border-radius: 3px;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 150px;
  overflow-y: auto;
}

.errand-result {
  margin-top: 0.3rem;
  padding: 0.35rem 0.5rem;
  background: hsl(var(--af-success) / 0.06);
  border-radius: 4px;
  border-left: 3px solid hsl(var(--af-success));
}

.errand-result-label {
  font-size: 0.73rem;
  font-weight: 600;
  color: hsl(var(--af-success));
  text-transform: uppercase;
  margin-bottom: 0.2rem;
}

.errand-result-text {
  font-size: 0.82rem;
  color: var(--af-fg);
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
}

/* Errand toggle button in header */
.errand-toggle-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  background: hsl(var(--muted-foreground) / 0.08);
  border: 1px solid var(--af-border);
  border-radius: 4px;
  padding: 0.2rem 0.5rem;
  font-size: 0.78rem;
  color: var(--af-fg);
  cursor: pointer;
}

.errand-toggle-btn:hover {
  background: hsl(var(--muted-foreground) / 0.14);
}

.errand-toggle-badge {
  background: var(--af-primary);
  color: white;
  font-size: 0.68rem;
  font-weight: 600;
  padding: 0.05rem 0.3rem;
  border-radius: 10px;
}

.relay-toggle-btn {
  background: hsl(var(--af-warning) / 0.12);
  border-color: hsl(var(--af-warning) / 0.3);
}

.relay-toggle-btn:hover {
  background: hsl(var(--af-warning) / 0.2);
}

/* ─── Relay Card ───────────────────────────────────────────────────────────── */

.relay-card {
  background: linear-gradient(135deg, hsl(var(--af-warning) / 0.06), hsl(var(--af-warning) / 0.02));
  border: 1px solid hsl(var(--af-warning) / 0.2);
  border-radius: 6px;
  overflow: hidden;
  padding: 0.35rem 0.6rem;
}

.relay-header {
  display: flex;
  align-items: center;
  gap: 0.4rem;
}

.relay-icon {
  font-size: 0.9rem;
}

.relay-name {
  font-size: 0.84rem;
  font-weight: 500;
  color: var(--af-fg);
  flex: 1;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.relay-status {
  font-size: 0.73rem;
  font-weight: 500;
  text-transform: capitalize;
}

.relay-status.started { color: hsl(var(--af-muted)); }
.relay-status.running { color: hsl(var(--af-info)); }
.relay-status.gate_waiting { color: hsl(var(--af-warning)); }
.relay-status.completed { color: hsl(var(--af-success)); }
.relay-status.failed { color: hsl(var(--af-error)); }

.relay-view-btn {
  background: hsl(var(--af-warning) / 0.15);
  border: 1px solid hsl(var(--af-warning) / 0.3);
  border-radius: 4px;
  padding: 0.1rem 0.4rem;
  font-size: 0.72rem;
  color: hsl(var(--af-warning));
  cursor: pointer;
}

.relay-view-btn:hover {
  background: hsl(var(--af-warning) / 0.25);
}

.relay-task {
  font-size: 0.8rem;
  color: var(--af-muted);
  margin-top: 0.2rem;
  font-style: italic;
}

.relay-summary {
  font-size: 0.82rem;
  color: var(--af-fg);
  margin-top: 0.3rem;
  padding: 0.3rem 0.4rem;
  background: hsl(var(--muted-foreground) / 0.04);
  border-radius: 4px;
}

.typing-dots {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  padding: 4px 0;
}

.typing-dots span {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--af-muted);
  animation: bounce 1.4s infinite ease-in-out both;
}

.typing-dots span:nth-child(1) { animation-delay: 0s; }
.typing-dots span:nth-child(2) { animation-delay: 0.16s; }
.typing-dots span:nth-child(3) { animation-delay: 0.32s; }

@keyframes bounce {
  0%, 80%, 100% { transform: scale(0.6); opacity: 0.4; }
  40% { transform: scale(1); opacity: 1; }
}

/* ─── Mobile Responsive ───────────────────────────────────────────────────── */

@media (max-width: 768px) {
  .session-sidebar {
    position: fixed;
    left: 0;
    top: 0;
    bottom: 0;
    z-index: 50;
    background: var(--af-bg);
    box-shadow: 2px 0 8px rgba(0, 0, 0, 0.1);
  }

  .session-sidebar.collapsed {
    width: 0;
    margin-left: 0;
    overflow: hidden;
  }

  .sidebar-collapse-btn {
    display: none;
  }

  .chats-header {
    padding: 0.5rem 0.75rem;
  }

  .header-search {
    min-width: 120px;
  }

  .message.user {
    max-width: 92%;
  }
}

/* ─── Input Bar ───────────────────────────────────────────────────────────── */

.chats-input-bar {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 0.6rem 1rem 0.75rem;
  flex-shrink: 0;
}

.input-inner {
  width: 100%;
  max-width: 960px;
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
}

.input-row {
  display: flex;
  align-items: flex-end;
  gap: 0.5rem;
}

.input-compose {
  flex: 1;
  position: relative;
  background: hsl(var(--muted-foreground) / 0.04);
  border: 1px solid hsl(var(--primary) / 0.18);
  border-radius: 20px;
  min-height: 80px;
  max-height: 180px;
  transition: border-color 0.15s, background 0.15s, box-shadow 0.15s;
}

.input-compose:focus-within {
  border-color: hsl(var(--primary) / 0.45);
  background: var(--af-bg);
  box-shadow: 0 0 0 3px hsl(var(--primary) / 0.08);
}

.input-backdrop {
  position: absolute;
  inset: 0;
  padding: 0.55rem 1rem;
  font-size: 0.98rem;
  font-family: inherit;
  line-height: 1.5;
  white-space: pre-wrap;
  word-wrap: break-word;
  overflow: hidden;
  color: transparent;
  pointer-events: none;
}

.input-backdrop :deep(.inline-mention) {
  display: inline;
  font-weight: 600;
  color: transparent;
  background: hsl(var(--primary) / 0.12);
  border-radius: 4px;
  padding: 0 3px;
}

.input-compose .chats-input {
  position: relative;
  display: block;
  width: 100%;
  height: 100%;
  min-height: 80px;
  max-height: 180px;
  padding: 0.55rem 1rem;
  border: none;
  background: transparent;
  color: var(--af-fg);
  font-size: 0.98rem;
  font-family: inherit;
  line-height: 1.5;
  resize: none;
  outline: none;
  box-shadow: none !important;
  overflow-y: auto;
}

.chats-input::placeholder {
  color: var(--af-muted);
}

.chats-input:disabled {
  opacity: 0.5;
}

.send-btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  background: linear-gradient(135deg, var(--vp-c-brand-1) 0%, var(--vp-c-brand-2) 100%);
  border: none;
  border-radius: 50%;
  color: #fff;
  cursor: pointer;
  transition: opacity 0.15s, transform 0.1s;
  flex-shrink: 0;
}

.send-btn:hover:not(:disabled) {
  opacity: 0.85;
}

.send-btn:active:not(:disabled) {
  transform: scale(0.95);
}

.send-btn:disabled {
  opacity: 0.3;
  cursor: not-allowed;
}



/* ─── Approval Gate ───────────────────────────────────────────────────────── */

.approval-gate {
  display: flex;
  flex-direction: column;
  gap: 0.6rem;
  padding: 0.75rem 1.25rem;
  border-top: 1px solid var(--af-border);
  flex-shrink: 0;
}

.approval-message {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.93rem;
  color: var(--af-fg);
}

.approval-icon {
  font-size: 1.18rem;
}

.approval-actions {
  display: flex;
  gap: 0.5rem;
}

.approve-btn,
.reject-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.35rem;
  padding: 0.4rem 0.9rem;
  border: none;
  border-radius: 6px;
  font-size: 0.88rem;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 0.15s;
}

.approve-btn {
  background: linear-gradient(135deg, var(--vp-c-brand-1) 0%, var(--vp-c-brand-2) 100%);
  color: #fff;
}

.reject-btn {
  background: transparent;
  color: var(--af-fg);
  border: 1px solid var(--af-border);
}

.approve-btn:hover,
.reject-btn:hover {
  opacity: 0.85;
}

/* ─── Approval Diff View ──────────────────────────────────────────────────── */

.approval-diff-list {
  display: flex;
  flex-direction: column;
  gap: 0.35rem;
  max-height: 300px;
  overflow-y: auto;
}

.diff-card {
  border: 1px solid var(--af-border);
  border-radius: 8px;
  overflow: hidden;
}

.diff-header {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.4rem 0.6rem;
  cursor: pointer;
  user-select: none;
}

.diff-header:hover {
  background: hsl(var(--muted-foreground) / 0.03);
}

.diff-title {
  font-size: 0.88rem;
  font-weight: 500;
  color: var(--af-fg);
  text-transform: capitalize;
  flex: 1;
}

.diff-status {
  font-size: 0.73rem;
  font-weight: 500;
  color: hsl(var(--af-warning));
}

.diff-status.approved {
  color: hsl(var(--af-success));
}

.diff-chevron {
  color: var(--af-muted);
}

.diff-body {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: 0.5rem;
  padding: 0.5rem 0.6rem;
  background: hsl(var(--muted-foreground) / 0.02);
  border-top: 1px solid var(--af-border);
}

.diff-side {
  display: flex;
  flex-direction: column;
  gap: 0.2rem;
}

.diff-label {
  font-size: 0.73rem;
  font-weight: 500;
  text-transform: uppercase;
  color: var(--af-muted);
  letter-spacing: 0.02em;
}

.diff-content {
  font-size: 0.83rem;
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  background: var(--af-bg);
  border: 1px solid var(--af-border);
  border-radius: 4px;
  padding: 0.35rem;
  overflow-x: auto;
  white-space: pre-wrap;
  word-break: break-word;
  color: var(--af-fg);
  margin: 0;
}

.diff-content.old {
  color: var(--af-muted);
}

.diff-editor {
  font-size: 0.83rem;
  font-family: 'JetBrains Mono', 'Fira Code', monospace;
  background: var(--af-bg);
  border: 1px solid var(--af-border);
  border-radius: 4px;
  padding: 0.35rem;
  color: var(--af-fg);
  resize: vertical;
  outline: none;
  width: 100%;
  box-sizing: border-box;
}

.diff-editor:focus {
  border-color: hsl(var(--primary) / 0.4);
}
</style>
