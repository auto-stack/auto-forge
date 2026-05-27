import { useEditor } from '@tiptap/vue-3'
import type { Editor } from '@tiptap/core'
import { createExtensions } from '../extensions'
import type { EditorOptions as AutoDownEditorOptions } from '../extensions'
import type { SlashItem } from '../menus/SlashMenu.vue'
import { preprocessMarkdown, applyTableAttrs } from '../extensions/tableAttributes'

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

  // Preprocess Markdown to extract table IAL attributes before parsing
  const { md: cleanContent, tableAttrs } = preprocessMarkdown(options.content)

  return useEditor({
    extensions,
    content: cleanContent,
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
      handleDOMEvents: {
        dblclick(view, event) {
          const target = event.target as HTMLElement
          const cellEl = target.closest('td, th') as HTMLElement | null
          const rowEl = target.closest('tr') as HTMLElement | null
          const threshold = 10

          // Double-click on column boundary → reset column width
          if (cellEl) {
            const rect = cellEl.getBoundingClientRect()
            const nearRightEdge = rect.right - event.clientX <= threshold && event.clientX <= rect.right + 2
            if (nearRightEdge) {
              const pos = view.posAtDOM(cellEl, 0)
              if (pos == null) return false
              const $pos = view.state.doc.resolve(pos)
              const cellNode = $pos.nodeAfter
              if (
                cellNode &&
                (cellNode.type.name === 'tableCell' || cellNode.type.name === 'tableHeader')
              ) {
                const tr = view.state.tr
                tr.setNodeMarkup($pos.pos, undefined, {
                  ...cellNode.attrs,
                  colwidth: null,
                })
                view.dispatch(tr)
                return true
              }
            }
          }

          // Double-click on row boundary → reset row height
          if (rowEl) {
            const rect = rowEl.getBoundingClientRect()
            const nearBottomEdge = event.clientY - rect.bottom >= -threshold && event.clientY >= rect.bottom - 2
            if (nearBottomEdge) {
              const pos = view.posAtDOM(rowEl, 0)
              if (pos == null) return false
              const $pos = view.state.doc.resolve(pos)
              const rowNode = $pos.nodeAfter
              if (rowNode && rowNode.type.name === 'tableRow') {
                const tr = view.state.tr
                tr.setNodeMarkup($pos.pos, undefined, {
                  ...rowNode.attrs,
                  rowheight: null,
                })
                view.dispatch(tr)
                return true
              }
            }
          }

          return false
        },
      },
    },
    onCreate: ({ editor }) => {
      // Apply extracted IAL attrs (colwidth/rowheight) to editor tables
      if (tableAttrs.length > 0) {
        // Use setTimeout to ensure editor is fully initialized
        setTimeout(() => applyTableAttrs(editor, tableAttrs), 0)
      }
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
