<template>
  <div
    v-if="visible"
    ref="menuRef"
    class="autodown-slash-menu"
    :style="positionStyle"
  >
    <div class="autodown-slash-menu-items">
      <button
        v-for="(item, index) in filteredItems"
        :key="item.title"
        class="autodown-slash-menu-item"
        :class="{ active: index === selectedIndex }"
        @click="selectItem(index)"
        @mouseenter="selectedIndex = index"
      >
        <component :is="item.icon" class="autodown-slash-menu-icon" :size="16" />
        <div class="autodown-slash-menu-info">
          <div class="autodown-slash-menu-title">{{ item.title }}</div>
          <div class="autodown-slash-menu-desc">{{ item.description }}</div>
        </div>
      </button>
      <div v-if="filteredItems.length === 0" class="autodown-slash-menu-empty">
        No results
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted, type Component } from 'vue'
import type { Editor, Range } from '@tiptap/core'

export interface SlashItem {
  title: string
  description: string
  icon: Component
  searchTerms: string[]
  command: (ctx: { editor: Editor; range: Range }) => void
}

const props = defineProps<{
  editor: Editor
  items: SlashItem[]
}>()

const visible = ref(false)
const query = ref('')
const range = ref<Range | null>(null)
const selectedIndex = ref(0)
const menuRef = ref<HTMLDivElement>()
const positionStyle = ref<Record<string, string>>({})

const filteredItems = computed(() => {
  const q = query.value.toLowerCase()
  if (!q) return props.items
  return props.items.filter((item) => {
    const terms = [item.title, item.description, ...item.searchTerms].join(' ').toLowerCase()
    return terms.includes(q)
  })
})

watch(filteredItems, () => {
  selectedIndex.value = 0
})

function updatePosition() {
  if (!range.value || !props.editor.view) return
  const coords = props.editor.view.coordsAtPos(range.value.from)
  const editorEl = props.editor.view.dom.closest('.autodown-editor') as HTMLElement | null
  if (!editorEl) return
  const rect = editorEl.getBoundingClientRect()
  positionStyle.value = {
    top: `${coords.bottom - rect.top + 8}px`,
    left: `${coords.left - rect.left}px`,
  }
}

function selectItem(index: number) {
  const item = filteredItems.value[index]
  if (!item || !range.value) return
  item.command({ editor: props.editor, range: range.value })
  close()
}

function close() {
  visible.value = false
  query.value = ''
  range.value = null
  selectedIndex.value = 0
}

function scrollActiveIntoView() {
  nextTick(() => {
    const menu = menuRef.value
    if (!menu) return
    const active = menu.querySelector('.autodown-slash-menu-item.active') as HTMLElement | null
    if (active) {
      active.scrollIntoView({ block: 'nearest', behavior: 'auto' })
    }
  })
}

function markHandled() {
  const storage = (props.editor.storage as Record<string, any>)?.['slash-command']
  if (storage) storage.handled = true
}

function onKeyDown(event: KeyboardEvent) {
  if (!visible.value) return

  if (event.key === 'ArrowDown') {
    event.preventDefault()
    selectedIndex.value = (selectedIndex.value + 1) % filteredItems.value.length
    scrollActiveIntoView()
    markHandled()
    return
  }
  if (event.key === 'ArrowUp') {
    event.preventDefault()
    selectedIndex.value = (selectedIndex.value - 1 + filteredItems.value.length) % filteredItems.value.length
    scrollActiveIntoView()
    markHandled()
    return
  }
  if (event.key === 'Enter' || event.key === 'NumpadEnter') {
    event.preventDefault()
    selectItem(selectedIndex.value)
    markHandled()
    return
  }
  if (event.key === 'Escape') {
    event.preventDefault()
    close()
    markHandled()
    return
  }
}

function onSlashOpen(e: Event) {
  const detail = (e as CustomEvent).detail
  query.value = detail.query
  range.value = detail.range
  visible.value = true
  selectedIndex.value = 0
  nextTick(updatePosition)
}

function onSlashUpdate(e: Event) {
  const detail = (e as CustomEvent).detail
  query.value = detail.query
  range.value = detail.range
  nextTick(updatePosition)
}

function onSlashClose() {
  close()
}

function onSlashKeydown(e: Event) {
  const detail = (e as CustomEvent).detail
  onKeyDown(detail.event)
}

onMounted(() => {
  document.addEventListener('autodown:slash-open', onSlashOpen)
  document.addEventListener('autodown:slash-update', onSlashUpdate)
  document.addEventListener('autodown:slash-close', onSlashClose)
  document.addEventListener('autodown:slash-keydown', onSlashKeydown)
})

onUnmounted(() => {
  document.removeEventListener('autodown:slash-open', onSlashOpen)
  document.removeEventListener('autodown:slash-update', onSlashUpdate)
  document.removeEventListener('autodown:slash-close', onSlashClose)
  document.removeEventListener('autodown:slash-keydown', onSlashKeydown)
})
</script>
