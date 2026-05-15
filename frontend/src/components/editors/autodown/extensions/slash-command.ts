import { Extension } from '@tiptap/core'
import Suggestion from '@tiptap/suggestion'
import type { SuggestionOptions } from '@tiptap/suggestion'

export interface SlashCommandStorage {
  query: string
  range: { from: number; to: number } | null
  handled: boolean
}

export const SlashCommand = Extension.create<{ suggestion: SuggestionOptions }>({
  name: 'slash-command',

  addOptions() {
    return {
      suggestion: {
        char: '/',
        command: ({ editor, range, props }) => {
          props.command({ editor, range })
        },
      } as SuggestionOptions,
    }
  },

  addStorage() {
    return {
      query: '',
      range: null,
      handled: false,
    } as SlashCommandStorage
  },

  addProseMirrorPlugins() {
    const storage = this.storage as SlashCommandStorage

    return [
      Suggestion({
        ...this.options.suggestion,
        editor: this.editor,
        allowedPrefixes: null,
        render: () => {
          return {
            onStart: (props) => {
              storage.query = props.query
              storage.range = { from: props.range.from, to: props.range.to }
              document.dispatchEvent(
                new CustomEvent('autodown:slash-open', {
                  detail: { query: props.query, range: props.range, items: props.items },
                })
              )
            },
            onUpdate: (props) => {
              storage.query = props.query
              storage.range = { from: props.range.from, to: props.range.to }
              document.dispatchEvent(
                new CustomEvent('autodown:slash-update', {
                  detail: { query: props.query, range: props.range, items: props.items },
                })
              )
            },
            onKeyDown: (props) => {
              storage.handled = false
              document.dispatchEvent(
                new CustomEvent('autodown:slash-keydown', {
                  detail: { event: props.event },
                })
              )
              return storage.handled
            },
            onExit: () => {
              storage.query = ''
              storage.range = null
              document.dispatchEvent(new CustomEvent('autodown:slash-close'))
            },
          }
        },
      }),
    ]
  },
})
