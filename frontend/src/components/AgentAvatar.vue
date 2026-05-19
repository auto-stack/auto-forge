<template>
  <img
    v-if="resolvedImageUrl"
    class="agent-avatar image"
    :class="[size]"
    :src="resolvedImageUrl"
    :alt="title"
    :title="title"
  />
  <span
    v-else
    class="agent-avatar"
    :class="[size, professionId]"
    :style="{ background: bgColor, color: textColor }"
    :title="title"
  >
    {{ initials }}
  </span>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { useAgentConfigs } from '@/composables/useAgentConfigs'

const props = defineProps<{
  professionId: string
  name?: string
  agentId?: string
  imageUrl?: string
  size?: 'xs' | 'sm' | 'md' | 'lg'
}>()

const { getById, getByProfession } = useAgentConfigs()

const size = computed(() => props.size ?? 'md')

// Priority: explicit imageUrl prop > agentId lookup > professionId lookup
const resolvedImageUrl = computed(() => {
  if (props.imageUrl) return props.imageUrl
  if (props.agentId) {
    const cfg = getById(props.agentId)
    if (cfg?.avatar_url) return cfg.avatar_url
  }
  const cfg = getByProfession(props.professionId)
  return cfg?.avatar_url
})

const professionColors: Record<string, { h: number; s: number; l: number }> = {
  assistant:  { h: 25,  s: 80, l: 48 },
  advisor:    { h: 270, s: 60, l: 52 },
  architect:  { h: 205, s: 70, l: 50 },
  planner:    { h: 145, s: 65, l: 38 },
  coder:      { h: 340, s: 75, l: 52 },
  tester:     { h: 45,  s: 90, l: 42 },
  reviewer:   { h: 170, s: 65, l: 38 },
  documenter: { h: 220, s: 65, l: 52 },
  gofer:      { h: 160, s: 65, l: 42 },
}

const color = computed(() => {
  const c = professionColors[props.professionId]
  if (c) return c
  // Fallback: hash the professionId to a hue
  let hash = 0
  for (let i = 0; i < props.professionId.length; i++) {
    hash = props.professionId.charCodeAt(i) + ((hash << 5) - hash)
  }
  return { h: Math.abs(hash) % 360, s: 60, l: 48 }
})

const bgColor = computed(() => {
  const c = color.value
  return `hsl(${c.h} ${c.s}% ${c.l}%)`
})

const textColor = computed(() => {
  const c = color.value
  return c.l > 55 ? 'hsl(220 15% 12%)' : '#fff'
})

const initials = computed(() => {
  const source = props.name?.trim() || props.professionId
  return source.charAt(0).toUpperCase()
})

const title = computed(() => {
  return props.name ? `${props.name} (${props.professionId})` : props.professionId
})
</script>

<style scoped>
.agent-avatar {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  border-radius: 50%;
  font-weight: 600;
  line-height: 1;
  user-select: none;
  font-family: system-ui, -apple-system, sans-serif;
  overflow: hidden;
}

.agent-avatar.image {
  object-fit: cover;
}

.agent-avatar.xs {
  width: 18px;
  height: 18px;
  font-size: 0.65rem;
}

.agent-avatar.sm {
  width: 30px;
  height: 30px;
  font-size: 0.9rem;
}

.agent-avatar.md {
  width: 28px;
  height: 28px;
  font-size: 1rem;
}

.agent-avatar.lg {
  width: 48px;
  height: 48px;
  font-size: 1.4rem;
}
</style>
