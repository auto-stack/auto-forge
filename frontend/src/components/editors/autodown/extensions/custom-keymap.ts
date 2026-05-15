import { Extension } from '@tiptap/core'

export const CustomKeymap = Extension.create({
  name: 'custom-keymap',

  addKeyboardShortcuts() {
    return {
      'Mod-a': ({ editor }) => {
        const { state } = editor
        const { from, to } = state.selection
        const $from = state.doc.resolve(from)
        const nodeStart = $from.start()
        const nodeEnd = $from.end()

        const notExtended = from > nodeStart || to < nodeEnd
        if (notExtended) {
          editor.chain().focus().selectParentNode().run()
          return true
        }
        return false
      },
    }
  },
})
