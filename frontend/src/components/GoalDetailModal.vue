<template>
  <Teleport to="body">
    <Transition name="modal">
      <div v-if="item" class="modal-backdrop" @click="onBackdropClick">
        <div class="modal-dialog" @click.stop>
          <!-- Header -->
          <div class="modal-header">
            <div class="modal-header-left">
              <span class="modal-id">{{ item.id }}</span>
              <span class="modal-title">{{ item.title }}</span>
            </div>
            <button class="modal-close" @click="$emit('close')">
              <X :size="16" />
            </button>
          </div>

          <!-- Meta bar -->
          <div v-if="!isEditing && hasMeta" class="modal-meta">
            <StatusBadge :status="item.status" />
            <span v-if="item.priority" class="meta-pill priority">
              {{ item.priority }}
            </span>
            <span v-for="tag in item.tags" :key="tag" class="meta-pill tag">
              {{ tag }}
            </span>
            <span v-if="item.depends_on?.length" class="meta-pill deps">
              Depends: {{ item.depends_on.join(', ') }}
            </span>
          </div>

          <!-- Body -->
          <div class="modal-body">
            <GoalEditor
              v-if="isEditing"
              :item="item"
              @save="$emit('save', $event)"
              @cancel="$emit('cancel-edit')"
            />
            <template v-else>
              <div class="description-section">
                <div class="section-label">Description</div>
                <GoalDetail :content="item.content || '(No content)'" @link-click="$emit('jump', $event)" />
              </div>
              <RelationsPanel
                :item="item"
                :project="project"
                @jump="$emit('jump', $event)"
              />
            </template>
          </div>

          <!-- Footer -->
          <div v-if="!isEditing" class="modal-footer">
            <StatusTransition
              :status="item.status"
              :section-type="sectionType"
              @change="$emit('status-change', { id: item.id, status: $event })"
            />
            <div class="modal-footer-actions">
              <button class="action-btn" @click="$emit('edit')">
                <Pencil :size="13" />
                Edit
              </button>
              <button class="action-btn danger" @click="$emit('delete', item.id)">
                <Trash2 :size="13" />
                Delete
              </button>
            </div>
          </div>
        </div>
      </div>
    </Transition>
  </Teleport>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted } from 'vue'
import type { SpecItem, SectionType, Status } from '@/types/specs'
import StatusBadge from './StatusBadge.vue'
import StatusTransition from './StatusTransition.vue'
import RelationsPanel from './RelationsPanel.vue'
import GoalDetail from './detail/GoalDetail.vue'
import GoalEditor from './editors/GoalEditor.vue'
import { X, Pencil, Trash2 } from 'lucide-vue-next'

const props = defineProps<{
  item: SpecItem | null
  project: string
  sectionType: SectionType
  isEditing: boolean
}>()

const emit = defineEmits<{
  close: []
  edit: []
  save: [payload: { title: string; content: string; priority: string; depends_on: string[] }]
  'cancel-edit': []
  'status-change': [payload: { id: string; status: Status }]
  delete: [id: string]
  jump: [id: string]
}>()

const hasMeta = computed(() =>
  props.item?.priority ||
  (props.item?.tags && props.item.tags.length > 0) ||
  (props.item?.depends_on && props.item.depends_on.length > 0)
)

function onKeydown(e: KeyboardEvent) {
  if (e.key === 'Escape') emit('close')
}

function onBackdropClick() {
  emit('close')
}

onMounted(() => {
  document.addEventListener('keydown', onKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', onKeydown)
})
</script>

<style scoped>
.modal-backdrop {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.35);
  backdrop-filter: blur(2px);
  padding: 1rem;
}

.modal-dialog {
  width: 100%;
  max-width: 680px;
  max-height: 85vh;
  display: flex;
  flex-direction: column;
  background: var(--af-bg);
  border-radius: 10px;
  box-shadow: 0 25px 50px -12px rgba(0, 0, 0, 0.25);
  border: 1px solid var(--af-border);
  overflow: hidden;
}

/* Entrance animation */
.modal-enter-active,
.modal-leave-active {
  transition: opacity 0.15s ease;
}
.modal-enter-active .modal-dialog,
.modal-leave-active .modal-dialog {
  transition: transform 0.15s ease, opacity 0.15s ease;
}
.modal-enter-from,
.modal-leave-to {
  opacity: 0;
}
.modal-enter-from .modal-dialog,
.modal-leave-to .modal-dialog {
  opacity: 0;
  transform: scale(0.97);
}

/* Header */
.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--af-border);
  flex-shrink: 0;
}

.modal-header-left {
  display: flex;
  align-items: center;
  gap: 0.6rem;
  min-width: 0;
}

.modal-id {
  font-family: ui-monospace, SFMono-Regular, Menlo, Monaco, Consolas, monospace;
  font-size: 0.8rem;
  font-weight: 600;
  color: var(--af-muted);
  background: hsl(var(--muted-foreground) / 0.08);
  padding: 0.15rem 0.4rem;
  border-radius: 4px;
  white-space: nowrap;
}

.modal-title {
  font-size: 1rem;
  font-weight: 600;
  color: var(--af-fg);
  line-height: 1.3;
}

.modal-close {
  background: none;
  border: none;
  color: var(--af-muted);
  cursor: pointer;
  padding: 0.25rem;
  border-radius: 4px;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
}
.modal-close:hover {
  color: var(--af-fg);
  background: hsl(var(--muted-foreground) / 0.08);
}

/* Meta bar */
.modal-meta {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.4rem;
  padding: 0.6rem 1.25rem;
  border-bottom: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.02);
  flex-shrink: 0;
}

.meta-pill {
  display: inline-flex;
  align-items: center;
  gap: 0.2rem;
  padding: 0.15rem 0.45rem;
  font-size: 0.78rem;
  border-radius: 6px;
  background: hsl(var(--muted-foreground) / 0.06);
  color: var(--af-muted);
  border: 1px solid var(--af-border);
}
.meta-pill.priority {
  background: hsl(var(--primary) / 0.08);
  color: hsl(var(--primary));
  border-color: hsl(var(--primary) / 0.2);
}
.meta-pill.tag {
  background: hsl(var(--accent) / 0.08);
  color: hsl(var(--accent));
  border-color: hsl(var(--accent) / 0.2);
}

/* Body */
.modal-body {
  flex: 1;
  overflow-y: auto;
  padding: 1rem 1.25rem;
}

.description-section {
  margin-bottom: 1.25rem;
}
.section-label {
  font-size: 0.75rem;
  font-weight: 700;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--af-muted);
  margin-bottom: 0.5rem;
}
.description-section :deep(.markdown-content) {
  font-size: 0.93rem;
  line-height: 1.65;
  color: var(--af-fg);
}
.description-section :deep(.markdown-content h1),
.description-section :deep(.markdown-content h2),
.description-section :deep(.markdown-content h3) {
  font-size: 0.93rem;
  font-weight: 400;
  line-height: 1.65;
  color: var(--af-fg);
  margin: 0;
  border: none;
  padding: 0;
}

/* Footer */
.modal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 1rem;
  padding: 0.75rem 1.25rem;
  border-top: 1px solid var(--af-border);
  flex-shrink: 0;
}

.modal-footer-actions {
  display: flex;
  align-items: center;
  gap: 0.5rem;
}

.action-btn {
  display: inline-flex;
  align-items: center;
  gap: 0.3rem;
  padding: 0.4rem 0.7rem;
  font-size: 0.83rem;
  border-radius: 6px;
  border: 1px solid var(--af-border);
  background: hsl(var(--muted-foreground) / 0.04);
  color: var(--af-fg);
  cursor: pointer;
  transition: all 0.12s;
}
.action-btn:hover {
  background: hsl(var(--muted-foreground) / 0.08);
}
.action-btn.danger {
  color: hsl(var(--destructive));
  border-color: hsl(var(--destructive) / 0.25);
  background: hsl(var(--destructive) / 0.04);
}
.action-btn.danger:hover {
  background: hsl(var(--destructive) / 0.08);
}
</style>
