import { useEditor } from '@tiptap/vue-3'
import type { Editor } from '@tiptap/core'
import { createExtensions } from '../extensions'
import type { EditorOptions as AutoDownEditorOptions } from '../extensions'
import type { SlashItem } from '../menus/SlashMenu.vue'

export interface UseAutoDownEditorOptions {
  content: string
  placeholder?: string
  editable?: boolean
  autofocus?: boolean
  slashItems?: SlashItem[]
  onUpdate?: (editor: Editor) => void
  onBlur?: (editor: Editor) => void
  onFocus?: (editor: Editor) => void
  onLinkClick?: (id: string) => void
}

export function useAutoDownEditor(options: UseAutoDownEditorOptions) {
  const extOptions: AutoDownEditorOptions = {
    placeholder: options.placeholder,
    slashItems: options.slashItems,
  }
  const extensions = createExtensions(extOptions)

  return useEditor({
    extensions,
    content: options.content,
    contentType: 'markdown',
    editable: options.editable ?? true,
    autofocus: options.autofocus ?? false,
    editorProps: {
      attributes: {
        class: 'autodown-editor-content',
      },
      handleClickOn: (view, pos, node, nodePos, event) => {
        const target = event.target as HTMLElement
        const anchor = target.closest('a')
        if (anchor && anchor.classList.contains('autodown-link')) {
          const href = anchor.getAttribute('href')
          if (href?.startsWith('#')) {
            options.onLinkClick?.(href.slice(1))
            return true
          }
        }
        return false
      },
    },
    onUpdate: ({ editor }) => {
      options.onUpdate?.(editor)
    },
    onBlur: ({ editor }) => {
      options.onBlur?.(editor)
    },
    onFocus: ({ editor }) => {
      options.onFocus?.(editor)
    },
  })
}
