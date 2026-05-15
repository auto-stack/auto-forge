<template>
  <div class="autodown-editor" :class="{ 'is-focused': focused }">
    <EditorContent :editor="editor" class="autodown-editor-content-wrapper" />
    <BubbleMenuVue v-if="editor" :editor="editor" />
    <SlashMenu v-if="editor" :editor="editor" :items="slashItems" />
    <TableMenu v-if="editor" :editor="editor" />
    <div v-if="showActions" class="autodown-editor-actions">
      <button class="autodown-save-btn" @click="handleSave">
        <Check :size="13" />
        Save
      </button>
      <button class="autodown-cancel-btn" @click="emit('cancel')">
        <X :size="13" />
        Cancel
      </button>
    </div>
  </div>
</template>

<script setup lang="ts">
import { computed, watch } from 'vue'
import { EditorContent } from '@tiptap/vue-3'
import { useAutoDownEditor } from '../composables/useAutoDownEditor'
import BubbleMenuVue from '../menus/BubbleMenu.vue'
import SlashMenu, { type SlashItem } from '../menus/SlashMenu.vue'
import TableMenu from '../menus/TableMenu.vue'
import {
  Heading1,
  Heading2,
  Heading3,
  Text,
  List,
  ListOrdered,
  CheckSquare,
  Code,
  Quote,
  Minus,
  Image,
  Table as TableIcon,
  Check,
  X,
} from 'lucide-vue-next'

const props = withDefaults(defineProps<{
  content: string
  placeholder?: string
  canEdit?: boolean
  autofocus?: boolean
  showActions?: boolean
}>(), {
  canEdit: true,
  autofocus: false,
  showActions: true,
})

const emit = defineEmits<{
  update: [markdown: string]
  save: [markdown: string]
  cancel: []
  blur: []
  focus: []
  'link-click': [id: string]
}>()

const slashItems: SlashItem[] = [
  {
    title: 'Text',
    description: 'Plain text',
    icon: Text,
    searchTerms: ['p'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).setParagraph().run(),
  },
  {
    title: 'Heading 1',
    description: 'Big section heading',
    icon: Heading1,
    searchTerms: ['h1'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).setHeading({ level: 1 }).run(),
  },
  {
    title: 'Heading 2',
    description: 'Medium section heading',
    icon: Heading2,
    searchTerms: ['h2'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).setHeading({ level: 2 }).run(),
  },
  {
    title: 'Heading 3',
    description: 'Small section heading',
    icon: Heading3,
    searchTerms: ['h3'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).setHeading({ level: 3 }).run(),
  },
  {
    title: 'Bullet List',
    description: 'Bullet list',
    icon: List,
    searchTerms: ['ul'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).toggleBulletList().run(),
  },
  {
    title: 'Numbered List',
    description: 'Numbered list',
    icon: ListOrdered,
    searchTerms: ['ol'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).toggleOrderedList().run(),
  },
  {
    title: 'Task List',
    description: 'Task list',
    icon: CheckSquare,
    searchTerms: ['task'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).toggleTaskList().run(),
  },
  {
    title: 'Code Block',
    description: 'Code snippet',
    icon: Code,
    searchTerms: ['code'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).setCodeBlock().run(),
  },
  {
    title: 'Quote',
    description: 'Quote',
    icon: Quote,
    searchTerms: ['blockquote'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).toggleBlockquote().run(),
  },
  {
    title: 'Divider',
    description: 'Horizontal rule',
    icon: Minus,
    searchTerms: ['hr'],
    command: ({ editor, range }) => editor.chain().focus().deleteRange(range).setHorizontalRule().run(),
  },
  {
    title: 'Image',
    description: 'Embed image',
    icon: Image,
    searchTerms: ['img'],
    command: ({ editor, range }) => {
      const url = window.prompt('Enter image URL')
      if (url) editor.chain().focus().deleteRange(range).setImage({ src: url }).run()
    },
  },
  {
    title: 'Table',
    description: 'Add table',
    icon: TableIcon,
    searchTerms: ['table'],
    command: ({ editor, range }) =>
      editor.chain().focus().deleteRange(range).insertTable({ rows: 3, cols: 3, withHeaderRow: true }).run(),
  },
]

const editor = useAutoDownEditor({
  content: props.content,
  placeholder: props.placeholder,
  editable: props.canEdit,
  autofocus: props.autofocus ?? false,
  slashItems,
  onUpdate: (editorInstance) => {
    emit('update', editorInstance.getMarkdown())
  },
  onBlur: () => {
    emit('blur')
  },
  onFocus: () => {
    emit('focus')
  },
  onLinkClick: (id: string) => {
    emit('link-click', id)
  },
})

const focused = computed(() => editor.value?.isFocused ?? false)

watch(
  () => props.content,
  (newContent) => {
    if (editor.value && editor.value.getMarkdown() !== newContent) {
      editor.value.commands.setContent(newContent, { emitUpdate: false, contentType: 'markdown' })
    }
  }
)

function handleSave() {
  if (editor.value) {
    emit('save', editor.value.getMarkdown())
  }
}

defineExpose({
  editor,
  handleSave,
})
</script>
