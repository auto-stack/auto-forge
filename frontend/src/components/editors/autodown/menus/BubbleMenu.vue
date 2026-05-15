<template>
  <BubbleMenu
    v-if="editor"
    :editor="editor"
    :options="{ placement: 'top' }"
    :should-show="shouldShow"
    class="autodown-bubble-menu"
  >
    <button
      v-for="btn in buttons"
      :key="btn.name"
      class="autodown-bubble-btn"
      :class="{ active: btn.isActive() }"
      :title="btn.title"
      @click="btn.action()"
    >
      <component :is="btn.icon" :size="14" />
    </button>
  </BubbleMenu>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import { BubbleMenu } from '@tiptap/vue-3/menus'
import type { Editor, isNodeSelection } from '@tiptap/core'
import {
  Bold,
  Italic,
  Strikethrough,
  Code,
  Link as LinkIcon,
  Underline,
  type LucideIcon,
} from 'lucide-vue-next'

const props = defineProps<{
  editor: Editor
}>()

function shouldShow({ editor, state }: { editor: Editor; state: Editor['state'] }) {
  const { selection } = state
  const { empty } = selection
  // @ts-expect-error isNodeSelection may not be exported directly
  const isNode = typeof isNodeSelection === 'function' ? isNodeSelection(selection) : false
  if (!editor.isEditable || empty || isNode || editor.isActive('image')) {
    return false
  }
  return true
}

interface BubbleButton {
  name: string
  title: string
  icon: LucideIcon
  isActive: () => boolean
  action: () => void
}

const buttons = computed<BubbleButton[]>(() => {
  const e = props.editor
  if (!e) return []
  return [
    {
      name: 'bold',
      title: 'Bold',
      icon: Bold,
      isActive: () => e.isActive('bold'),
      action: () => e.chain().focus().toggleBold().run(),
    },
    {
      name: 'italic',
      title: 'Italic',
      icon: Italic,
      isActive: () => e.isActive('italic'),
      action: () => e.chain().focus().toggleItalic().run(),
    },
    {
      name: 'underline',
      title: 'Underline',
      icon: Underline,
      isActive: () => e.isActive('underline'),
      action: () => e.chain().focus().toggleUnderline().run(),
    },
    {
      name: 'strike',
      title: 'Strikethrough',
      icon: Strikethrough,
      isActive: () => e.isActive('strike'),
      action: () => e.chain().focus().toggleStrike().run(),
    },
    {
      name: 'code',
      title: 'Inline Code',
      icon: Code,
      isActive: () => e.isActive('code'),
      action: () => e.chain().focus().toggleCode().run(),
    },
    {
      name: 'link',
      title: 'Link',
      icon: LinkIcon,
      isActive: () => e.isActive('link'),
      action: () => {
        if (e.isActive('link')) {
          e.chain().focus().unsetLink().run()
        } else {
          const url = window.prompt('Enter URL')
          if (url) {
            e.chain().focus().setLink({ href: url }).run()
          }
        }
      },
    },
  ]
})
</script>
