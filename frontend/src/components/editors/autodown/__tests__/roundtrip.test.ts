import { describe, it, expect } from 'vitest'
import { Editor } from '@tiptap/core'
import { createExtensions } from '../extensions'

function createTestEditor(content: string) {
  const extensions = createExtensions()
  const editor = new Editor({
    extensions,
    content,
    contentType: 'markdown',
  })
  return editor
}

describe('AutoDown Editor Markdown Round-trip', () => {
  it('round-trips a simple paragraph', () => {
    const md = 'Hello world'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips headings', () => {
    const md = '# Heading 1\n\n## Heading 2\n\n### Heading 3'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips bold and italic', () => {
    const md = '**bold** and *italic*'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips a bullet list', () => {
    const md = '- Item 1\n- Item 2\n- Item 3'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips a numbered list', () => {
    const md = '1. First\n2. Second\n3. Third'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips a code block', () => {
    const md = '```\nconst x = 1\n```'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips a blockquote', () => {
    const md = '> This is a quote'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips a link', () => {
    const md = '[link](https://example.com)'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips an image', () => {
    const md = '![alt](https://example.com/img.png)'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips a horizontal rule', () => {
    const md = '---'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips a task list', () => {
    const md = '- [ ] Todo\n- [x] Done'
    const editor = createTestEditor(md)
    expect(editor.getMarkdown().trim()).toBe(md)
    editor.destroy()
  })

  it('round-trips a table', () => {
    const md = '| Header 1 | Header 2 |\n| --- | --- |\n| Cell 1 | Cell 2 |'
    const editor = createTestEditor(md)
    const result = editor.getMarkdown().trim()
    // Table serializers may pad column widths; check structure instead
    expect(result).toContain('Header 1')
    expect(result).toContain('Header 2')
    expect(result).toContain('Cell 1')
    expect(result).toContain('Cell 2')
    editor.destroy()
  })

  it('round-trips mixed content', () => {
    const md = `# Title

Some **bold** text and a [link](https://example.com).

- List item 1
- List item 2

> A blockquote

\`\`\`ts
const x = 1
\`\`\`
`
    const editor = createTestEditor(md)
    const result = editor.getMarkdown().trim()
    expect(result).toContain('# Title')
    expect(result).toContain('**bold**')
    expect(result).toContain('[link](https://example.com)')
    expect(result).toContain('- List item 1')
    expect(result).toContain('> A blockquote')
    expect(result).toContain('const x = 1')
    editor.destroy()
  })
})
