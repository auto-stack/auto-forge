import { Extension } from '@tiptap/core'
import { rowResizing } from './rowResizing'

export const RowResizingExtension = Extension.create({
  name: 'rowResizing',
  addProseMirrorPlugins() {
    return [rowResizing()]
  },
})
