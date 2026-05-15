import { describe, it, expect } from 'vitest'
import { Editor } from '@tiptap/core'
import { createExtensions } from '../extensions'

describe('Editor mount', () => {
  it('creates an editable editor with content', () => {
    const extensions = createExtensions()
    const editor = new Editor({
      extensions,
      content: 'Hello world',
      contentType: 'markdown',
    })

    expect(editor).toBeDefined()
    expect(editor.isEditable).toBe(true)
    expect(editor.getText()).toBe('Hello world')

    editor.destroy()
  })

  it('parses markdown content correctly', () => {
    const extensions = createExtensions()
    const editor = new Editor({
      extensions,
      content: '# Heading\n\n**bold** text',
      contentType: 'markdown',
    })

    expect(editor.isActive('heading', { level: 1 })).toBe(true)
    expect(editor.getText()).toContain('Heading')
    expect(editor.getText()).toContain('bold text')

    editor.destroy()
  })
})
