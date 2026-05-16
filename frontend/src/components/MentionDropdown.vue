<template>
  <Teleport to="body">
    <div v-if="visible && filtered.length > 0" class="mention-dropdown" :style="position">
      <button
        v-for="(prof, i) in filtered"
        :key="prof.id"
        class="mention-item"
        :class="{ active: i === index }"
        @click="$emit('select', prof.id)"
        @mouseenter="index = i"
      >
        <AgentAvatar :profession-id="prof.id" :name="prof.name" size="sm" />
        <span class="mention-name">@{{ prof.id }}</span>
        <span class="mention-label">{{ prof.name }}</span>
      </button>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { ref, computed, watch } from 'vue'
import AgentAvatar from './AgentAvatar.vue'

export interface ProfessionOption {
  id: string
  name: string
}

const props = defineProps<{
  professions: ProfessionOption[]
  visible: boolean
  filter: string
  anchorRect: DOMRect | null
}>()

defineEmits<{
  select: [id: string]
}>()

const index = ref(0)

const filtered = computed(() => {
  const f = props.filter.toLowerCase()
  return props.professions.filter(p =>
    p.id.toLowerCase().includes(f) || p.name.toLowerCase().includes(f)
  )
})

const position = computed(() => {
  if (!props.anchorRect) return {}
  return {
    position: 'fixed' as const,
    left: `${props.anchorRect.left}px`,
    bottom: `${window.innerHeight - props.anchorRect.top + 4}px`,
  }
})

watch(filtered, () => { index.value = 0 })

defineExpose({
  moveUp() { index.value = Math.max(0, index.value - 1) },
  moveDown() { index.value = Math.min(filtered.value.length - 1, index.value + 1) },
  currentId(): string | undefined { return filtered.value[index.value]?.id },
  hasItems(): boolean { return filtered.value.length > 0 },
})
</script>

<style scoped>
.mention-dropdown {
  min-width: 180px;
  max-height: 220px;
  overflow-y: auto;
  background: var(--af-card);
  border: 1px solid var(--af-border);
  border-radius: 8px;
  box-shadow: 0 4px 16px rgba(0, 0, 0, 0.12);
  padding: 4px;
  z-index: 200;
}

.mention-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 6px 10px;
  border: none;
  border-radius: 5px;
  background: transparent;
  color: var(--af-fg);
  font-size: 0.88rem;
  cursor: pointer;
  text-align: left;
  transition: background 0.1s;
}

.mention-item:hover,
.mention-item.active {
  background: hsl(var(--primary) / 0.08);
}

.mention-item.active {
  color: var(--af-primary);
}

.mention-name {
  font-weight: 600;
  font-family: monospace;
}

.mention-label {
  color: var(--af-muted);
  font-size: 0.83rem;
}
</style>
