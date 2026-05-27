import { type AnyExtension } from '@tiptap/core'
import StarterKit from '@tiptap/starter-kit'
import Placeholder from '@tiptap/extension-placeholder'
import Image from '@tiptap/extension-image'
import Link from '@tiptap/extension-link'
import TaskList from '@tiptap/extension-task-list'
import TaskItem from '@tiptap/extension-task-item'
import { Table } from '@tiptap/extension-table'
import TableCell from '@tiptap/extension-table-cell'
import TableHeader from '@tiptap/extension-table-header'
import { Markdown } from '@tiptap/markdown'
import DragHandle from '@tiptap/extension-drag-handle'

import { SlashCommand } from './slash-command'
import { CustomKeymap } from './custom-keymap'
import { CustomTableRow } from './CustomTableRow'
import { RowResizingExtension } from './RowResizingExtension'
import type { SlashItem } from '../menus/SlashMenu.vue'

export interface EditorOptions {
  placeholder?: string
  slashItems?: SlashItem[]
}

export function createExtensions(options: EditorOptions = {}): AnyExtension[] {
  return [
    StarterKit.configure({
      heading: { levels: [1, 2, 3] },
      link: false,
    }),
    Placeholder.configure({
      placeholder: options.placeholder ?? "Type '/' for commands…",
    }),
    Link.configure({
      openOnClick: false,
      HTMLAttributes: { class: 'autodown-link' },
    }),
    Image.configure({
      allowBase64: true,
      HTMLAttributes: { class: 'autodown-image' },
    }),
    TaskList.configure({
      HTMLAttributes: { class: 'autodown-task-list' },
    }),
    TaskItem.configure({
      nested: true,
      HTMLAttributes: { class: 'autodown-task-item' },
    }),
    Table.configure({
      resizable: true,
      handleWidth: 8,
      cellMinWidth: 60,
      HTMLAttributes: { class: 'autodown-table' },
    }),
    CustomTableRow,
    TableHeader,
    TableCell,
    RowResizingExtension,
    Markdown.configure({
      indentation: { style: 'space', size: 2 },
    }),
    SlashCommand.configure({
      suggestion: {
        items: ({ query }: { query: string }) => {
          const items = options.slashItems ?? []
          if (!query) return items
          const q = query.toLowerCase()
          return items.filter((item) => {
            const terms = [item.title, item.description, ...item.searchTerms].join(' ').toLowerCase()
            return terms.includes(q)
          })
        },
      } as any,
    }),
    DragHandle.configure({
      render() {
        const el = document.createElement('div')
        el.className = 'autodown-drag-handle'
        el.innerHTML = '<svg xmlns="http://www.w3.org/2000/svg" width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="9" cy="12" r="1"/><circle cx="9" cy="5" r="1"/><circle cx="9" cy="19" r="1"/><circle cx="15" cy="12" r="1"/><circle cx="15" cy="5" r="1"/><circle cx="15" cy="19" r="1"/></svg>'
        return el
      },
    }),
    CustomKeymap,
  ]
}
