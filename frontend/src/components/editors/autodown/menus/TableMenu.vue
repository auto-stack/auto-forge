<template>
  <div
    v-if="visible"
    ref="menuRef"
    class="autodown-table-menu"
    :style="positionStyle"
  >
    <div class="autodown-table-menu-group">
      <button class="autodown-table-menu-btn" title="Add row above" @click="run('addRowBefore')">
        <ArrowUpToLine :size="13" />
      </button>
      <button class="autodown-table-menu-btn" title="Add row below" @click="run('addRowAfter')">
        <ArrowDownToLine :size="13" />
      </button>
      <button class="autodown-table-menu-btn" title="Add column left" @click="run('addColumnBefore')">
        <ArrowLeftToLine :size="13" />
      </button>
      <button class="autodown-table-menu-btn" title="Add column right" @click="run('addColumnAfter')">
        <ArrowRightToLine :size="13" />
      </button>
    </div>
    <div class="autodown-table-menu-divider" />
    <div class="autodown-table-menu-group">
      <button class="autodown-table-menu-btn" title="Delete row" @click="run('deleteRow')">
        <Trash2 :size="13" />
      </button>
      <button class="autodown-table-menu-btn" title="Delete column" @click="run('deleteColumn')">
        <Eraser :size="13" />
      </button>
      <button class="autodown-table-menu-btn danger" title="Delete table" @click="run('deleteTable')">
        <X :size="13" />
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch, nextTick, onMounted, onUnmounted } from 'vue'
import type { Editor } from '@tiptap/core'
import {
  ArrowUpToLine,
  ArrowDownToLine,
  ArrowLeftToLine,
  ArrowRightToLine,
  Trash2,
  Eraser,
  X,
} from 'lucide-vue-next'

const props = defineProps<{
  editor: Editor
}>()

const visible = ref(false)
const menuRef = ref<HTMLDivElement>()
const positionStyle = ref<Record<string, string>>({})

type TableCommand =
  | 'addColumnBefore'
  | 'addColumnAfter'
  | 'addRowBefore'
  | 'addRowAfter'
  | 'deleteColumn'
  | 'deleteRow'
  | 'deleteTable'

function run(command: TableCommand) {
  const chain = props.editor.chain().focus()
  let result = false
  switch (command) {
    case 'addColumnBefore':
      result = chain.addColumnBefore().run()
      break
    case 'addColumnAfter':
      result = chain.addColumnAfter().run()
      break
    case 'addRowBefore':
      result = chain.addRowBefore().run()
      break
    case 'addRowAfter':
      result = chain.addRowAfter().run()
      break
    case 'deleteColumn':
      result = chain.deleteColumn().run()
      break
    case 'deleteRow':
      result = chain.deleteRow().run()
      break
    case 'deleteTable':
      result = chain.deleteTable().run()
      break
  }
  // debug: console.log('[TableMenu]', command, 'result:', result, 'isActive(table):', props.editor.isActive('table'))
}

function updatePosition() {
  const { view, state } = props.editor
  const { selection } = state

  // Find the table DOM element
  const tableEl = view.dom.querySelector('.tableWrapper, table') as HTMLElement | null
  if (!tableEl) {
    visible.value = false
    return
  }

  const editorRect = view.dom.getBoundingClientRect()
  const tableRect = tableEl.getBoundingClientRect()

  // Position at the top-right of the table, relative to the editor
  positionStyle.value = {
    top: `${tableRect.top - editorRect.top - 8}px`,
    left: `${tableRect.right - editorRect.left - 180}px`,
  }
}

function checkVisibility() {
  const insideTable = props.editor.isActive('table')
  const wasVisible = visible.value
  visible.value = insideTable
  if (insideTable) {
    nextTick(updatePosition)
  }
}

let rafId: number | null = null
function scheduleCheck() {
  if (rafId) cancelAnimationFrame(rafId)
  rafId = requestAnimationFrame(() => {
    checkVisibility()
    rafId = null
  })
}

watch(
  () => props.editor?.state.selection,
  scheduleCheck,
  { immediate: true }
)

function handleOutsideClick(event: MouseEvent) {
  const target = event.target as HTMLElement
  const menu = menuRef.value
  const editorEl = props.editor.view.dom
  if (menu && !menu.contains(target) && !editorEl.contains(target)) {
    visible.value = false
  }
}

onMounted(() => {
  props.editor.on('selectionUpdate', scheduleCheck)
  document.addEventListener('mousedown', handleOutsideClick)
})

onUnmounted(() => {
  props.editor.off('selectionUpdate', scheduleCheck)
  document.removeEventListener('mousedown', handleOutsideClick)
})
</script>
