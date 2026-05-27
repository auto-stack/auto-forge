/**
 * AutoDown Table IAL (Inline Attribute List) utilities.
 *
 * Syntax: {cols:[120,"auto",200], rows:[40,"auto","auto"]}
 * - "auto" or missing value means no fixed width/height
 * - Stored in ProseMirror as colwidth: (number|null)[] and rowheight: number|null
 */

import type { Editor } from '@tiptap/core'
import { Transaction } from '@tiptap/pm/state'

export interface TableAttr {
  cols: (number | null)[]
  rows: (number | null)[]
}

const IAL_REGEX =
  /(\|[^\n]*\|[ \t]*\n\|[-:\| \t]+\|[ \t]*\n(?:\|[^\n]*\|[ \t]*\n)+)\{cols:\[(.*?)\](?:,\s*rows:\[(.*?)\])?\}[ \t]*(?:\n|$)/g

function parseValue(s: string): number | null {
  const trimmed = s.trim().replace(/^["']|["']$/g, '')
  if (trimmed === 'auto') return null
  const num = parseInt(trimmed, 10)
  return isNaN(num) ? null : num
}

function parseArray(s: string): (number | null)[] {
  return s.split(',').map(parseValue)
}

export function formatValue(v: number | null): string {
  return v === null ? '"auto"' : String(v)
}

export function formatArray(arr: (number | null)[]): string {
  return arr.map(formatValue).join(',')
}

export function hasAnyValue(arr: (number | null)[]): boolean {
  return arr.some((v) => v !== null)
}

/** Extract IAL attributes from Markdown and return cleaned Markdown + attrs. */
export function preprocessMarkdown(md: string): { md: string; tableAttrs: TableAttr[] } {
  const tableAttrs: TableAttr[] = []

  const cleaned = md.replace(IAL_REGEX, (_match, _tableBody, colsStr, rowsStr) => {
    tableAttrs.push({
      cols: parseArray(colsStr),
      rows: rowsStr ? parseArray(rowsStr) : [],
    })
    return _tableBody
  })

  return { md: cleaned, tableAttrs }
}

/** Apply extracted IAL attrs to editor tables (colwidth on cells, rowheight on rows). */
export function applyTableAttrs(editor: Editor, tableAttrs: TableAttr[]): void {
  if (tableAttrs.length === 0) return

  let tableIndex = 0
  const tr = editor.state.tr

  editor.state.doc.descendants((node, pos) => {
    if (node.type.name !== 'table') return
    const attrs = tableAttrs[tableIndex++]
    if (!attrs) return

    // Apply colwidth to cells in the first row (Tiptap stores colwidth per cell)
    const firstRow = node.firstChild
    if (firstRow && attrs.cols.length > 0) {
      let colIndex = 0
      let cellPos = pos + 2 // table open (1) + row open (1)

      firstRow.content.forEach((cell) => {
        const colspan = cell.attrs.colspan || 1
        const colwidths: (number | null)[] = []
        for (let i = 0; i < colspan; i++) {
          colwidths.push(attrs.cols[colIndex + i] ?? null)
        }
        if (hasAnyValue(colwidths)) {
          tr.setNodeMarkup(cellPos, undefined, {
            ...cell.attrs,
            colwidth: colwidths,
          })
        }
        colIndex += colspan
        cellPos += cell.nodeSize
      })
    }

    // Apply rowheight to rows
    if (attrs.rows.length > 0) {
      let rowPos = pos + 1 // table open tag
      let rowIndex = 0

      node.content.forEach((row) => {
        if (rowIndex < attrs.rows.length && attrs.rows[rowIndex] !== null) {
          tr.setNodeMarkup(rowPos, undefined, {
            ...row.attrs,
            rowheight: attrs.rows[rowIndex],
          })
        }
        rowIndex++
        rowPos += row.nodeSize
      })
    }
  })

  if (tr.docChanged) {
    editor.view.dispatch(tr)
  }
}

/** Build IAL string from colwidth/rowheight arrays. */
export function buildIAL(
  colwidth: (number | null)[],
  rowheight: (number | null)[]
): string | null {
  const hasCols = hasAnyValue(colwidth)
  const hasRows = hasAnyValue(rowheight)
  if (!hasCols && !hasRows) return null

  const parts: string[] = []
  if (hasCols) parts.push(`cols:[${formatArray(colwidth)}]`)
  if (hasRows) parts.push(`rows:[${formatArray(rowheight)}]`)
  return '{' + parts.join(', ') + '}\n'
}

/** Append IAL blocks to each table in Markdown string. */
export function appendTableIAL(md: string, editor: Editor): string {
  const tables: { colwidth: (number | null)[]; rowheight: (number | null)[] }[] = []

  editor.state.doc.descendants((node) => {
    if (node.type.name !== 'table') return

    // Extract colwidth from first row cells
    const colwidth: (number | null)[] = []
    const firstRow = node.firstChild
    if (firstRow) {
      firstRow.content.forEach((cell) => {
        const cw = cell.attrs.colwidth
        if (Array.isArray(cw)) {
          for (let i = 0; i < (cell.attrs.colspan || 1); i++) {
            colwidth.push(cw[i] ?? null)
          }
        } else {
          for (let i = 0; i < (cell.attrs.colspan || 1); i++) {
            colwidth.push(null)
          }
        }
      })
    }

    // Extract rowheight from rows
    const rowheight: (number | null)[] = []
    node.content.forEach((row) => {
      rowheight.push(row.attrs.rowheight ?? null)
    })

    tables.push({ colwidth, rowheight })
  })

  if (tables.length === 0) return md

  // Find each table block and append IAL
  const tableBlockRegex =
    /(\|[^\n]*\|[ \t]*\n\|[-:\| \t]+\|[ \t]*\n(?:\|[^\n]*\|[ \t]*\n)+)/g
  let result = ''
  let lastIndex = 0
  let tableIdx = 0
  let match: RegExpExecArray | null

  while ((match = tableBlockRegex.exec(md))) {
    result += md.slice(lastIndex, match.index + match[0].length)
    const ial = buildIAL(tables[tableIdx].colwidth, tables[tableIdx].rowheight)
    if (ial) {
      result += ial
    }
    lastIndex = match.index + match[0].length
    tableIdx++
    if (tableIdx >= tables.length) break
  }

  result += md.slice(lastIndex)
  return result
}
