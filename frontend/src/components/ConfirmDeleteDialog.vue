<template>
  <Teleport to="body">
    <div
      v-if="open"
      class="confirm-delete-overlay"
      role="alertdialog"
      aria-modal="true"
      :aria-label="t('chat.deleteAllSessions')"
      @keydown.escape="$emit('cancel')"
    >
      <div class="confirm-delete-dialog">
        <div class="confirm-delete-header">
          <span class="confirm-delete-icon">⚠️</span>
          <h3 class="confirm-delete-title">{{ t('chat.deleteAllSessions') }}</h3>
        </div>
        <div class="confirm-delete-body">
          <p>{{ t('chat.confirmDeleteAll') }}</p>
        </div>
        <div class="confirm-delete-actions">
          <button
            ref="cancelBtn"
            class="confirm-delete-cancel"
            :disabled="loading"
            @click="$emit('cancel')"
          >
            {{ t('chat.cancel') }}
          </button>
          <button
            class="confirm-delete-confirm"
            :disabled="loading"
            @click="$emit('confirm')"
          >
            <span v-if="loading" class="confirm-delete-spinner" />
            <span v-else>{{ t('chat.confirmDelete') }}</span>
          </button>
        </div>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, watch, nextTick } from 'vue'
import { useI18n } from 'vue-i18n'

const props = defineProps<{
  open: boolean
  loading?: boolean
}>()

defineEmits<{
  confirm: []
  cancel: []
}>()

const { t } = useI18n()
const cancelBtn = ref<HTMLButtonElement | null>(null)

watch(() => props.open, async (val) => {
  if (val) {
    await nextTick()
    cancelBtn.value?.focus()
  }
})
</script>

<style scoped>
.confirm-delete-overlay {
  position: fixed;
  inset: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: center;
  background: rgba(0, 0, 0, 0.5);
  backdrop-filter: blur(2px);
}

.confirm-delete-dialog {
  background: var(--af-bg, #1e1e2e);
  border-radius: 12px;
  padding: 24px;
  width: 400px;
  max-width: 90vw;
  box-shadow: 0 8px 32px rgba(0, 0, 0, 0.3);
  border: 1px solid hsl(0 60% 50% / 0.3);
}

.confirm-delete-header {
  display: flex;
  align-items: center;
  gap: 10px;
  margin-bottom: 16px;
}

.confirm-delete-icon {
  font-size: 20px;
}

.confirm-delete-title {
  margin: 0;
  font-size: 16px;
  font-weight: 600;
  color: hsl(0 70% 60%);
}

.confirm-delete-body {
  margin-bottom: 20px;
  color: var(--af-fg-muted, #a0a0b0);
  font-size: 14px;
  line-height: 1.5;
}

.confirm-delete-actions {
  display: flex;
  justify-content: flex-end;
  gap: 10px;
}

.confirm-delete-cancel {
  padding: 8px 16px;
  border-radius: 6px;
  border: 1px solid var(--af-border, #333);
  background: transparent;
  color: var(--af-fg-muted, #a0a0b0);
  cursor: pointer;
  font-size: 14px;
  transition: background 0.15s;
}

.confirm-delete-cancel:hover {
  background: hsl(var(--muted-foreground) / 0.08);
}

.confirm-delete-confirm {
  padding: 8px 16px;
  border-radius: 6px;
  border: none;
  background: hsl(0 60% 50%);
  color: white;
  cursor: pointer;
  font-size: 14px;
  font-weight: 500;
  transition: background 0.15s;
  display: flex;
  align-items: center;
  gap: 6px;
}

.confirm-delete-confirm:hover:not(:disabled) {
  background: hsl(0 70% 45%);
}

.confirm-delete-confirm:disabled {
  opacity: 0.6;
  cursor: not-allowed;
}

.confirm-delete-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid rgba(255, 255, 255, 0.3);
  border-top-color: white;
  border-radius: 50%;
  animation: spin 0.6s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
