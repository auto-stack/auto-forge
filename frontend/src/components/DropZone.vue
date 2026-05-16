<template>
  <div
    class="drop-zone"
    :class="{ active: isDragging }"
    @dragenter.prevent="isDragging = true"
    @dragover.prevent
    @dragleave.prevent="isDragging = false"
    @drop.prevent="handleDrop"
  >
    <UploadCloud :size="16" />
    <span class="drop-text">{{ isDragging ? 'Drop files here' : 'Drag files to upload' }}</span>
    <div v-if="uploadProgress !== null" class="progress-bar">
      <div class="progress-fill" :style="{ width: uploadProgress + '%' }" />
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import { UploadCloud } from 'lucide-vue-next'

defineProps<{
  uploadProgress: number | null
}>()

const emit = defineEmits<{
  drop: [files: File[]]
}>()

const isDragging = ref(false)

function handleDrop(e: DragEvent) {
  isDragging.value = false
  const files = Array.from(e.dataTransfer?.files ?? [])
  if (files.length > 0) emit('drop', files)
}
</script>

<style scoped>
.drop-zone {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.3rem;
  padding: 0.75rem;
  margin: 0.5rem;
  border: 1px dashed var(--af-border);
  border-radius: 6px;
  color: var(--af-muted);
  transition: all 0.15s;
  cursor: pointer;
}

.drop-zone.active {
  border-color: hsl(var(--primary));
  background: hsl(var(--primary) / 0.04);
  color: hsl(var(--primary));
}

.drop-text {
  font-size: 0.78rem;
}

.progress-bar {
  width: 100%;
  height: 3px;
  background: hsl(var(--muted-foreground) / 0.1);
  border-radius: 2px;
  overflow: hidden;
}

.progress-fill {
  height: 100%;
  background: hsl(var(--primary));
  transition: width 0.2s;
}
</style>
