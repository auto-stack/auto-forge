/**
 * ProseMirror plugin for row height resizing in tables.
 * Adds draggable handles at the bottom of each table row.
 */
import { Plugin, PluginKey } from '@tiptap/pm/state'
import { Decoration, DecorationSet } from '@tiptap/pm/view'

export const rowResizingPluginKey = new PluginKey('rowResizing')

interface PluginState {
  activeHandle: number // row index or -1
  dragging: { startY: number; startHeight: number; rowPos: number } | null
}

function findRowAtPoint(view: any, event: MouseEvent, threshold: number = 8): { rowEl: HTMLTableRowElement; rowIndex: number; rowPos: number; nearBottom: boolean } | null {
  const target = event.target as HTMLElement
  const rowEl = target.closest('tr') as HTMLTableRowElement | null
  if (!rowEl) return null

  const tableEl = rowEl.closest('table')
  if (!tableEl) return null

  // Check if we're near the bottom edge of the row
  const rect = rowEl.getBoundingClientRect()
  const nearBottom = event.clientY >= rect.bottom - threshold && event.clientY <= rect.bottom + threshold

  if (!nearBottom) return null

  const rows = Array.from(tableEl.querySelectorAll('tr'))
  const rowIndex = rows.indexOf(rowEl)
  if (rowIndex === -1) return null

  // Find ProseMirror position for this row
  const pos = view.posAtDOM(rowEl, 0)
  if (pos == null) return null

  return { rowEl, rowIndex, rowPos: pos, nearBottom }
}

export function rowResizing(options: { handleHeight?: number; minRowHeight?: number } = {}) {
  const minRowHeight = options.minRowHeight ?? 24

  return new Plugin({
    key: rowResizingPluginKey,

    state: {
      init(): PluginState {
        return { activeHandle: -1, dragging: null }
      },
      apply(tr, value): PluginState {
        const meta = tr.getMeta(rowResizingPluginKey)
        if (meta) {
          return { ...value, ...meta }
        }
        if (value.dragging && tr.docChanged) {
          return { activeHandle: -1, dragging: null }
        }
        return value
      },
    },

    props: {
      decorations(state) {
        const pluginState = rowResizingPluginKey.getState(state) as PluginState | undefined
        if (!pluginState || pluginState.activeHandle === -1) return DecorationSet.empty

        const decorations: Decoration[] = []

        state.doc.descendants((node, pos) => {
          if (node.type.name !== 'table') return

          let rowPos = pos + 1
          let rowIndex = 0

          node.content.forEach((row) => {
            if (rowIndex === pluginState.activeHandle || pluginState.dragging?.rowPos === rowPos) {
              decorations.push(
                Decoration.widget(rowPos + row.nodeSize - 1, () => {
                  const handle = document.createElement('div')
                  handle.className = 'autodown-row-resize-handle'
                  if (pluginState.dragging?.rowPos === rowPos) {
                    handle.classList.add('active')
                  }
                  return handle
                }, { side: -1 })
              )
            }
            rowIndex++
            rowPos += row.nodeSize
          })
        })

        return DecorationSet.create(state.doc, decorations)
      },

      attributes(state) {
        const pluginState = rowResizingPluginKey.getState(state) as PluginState | undefined
        if (pluginState && pluginState.activeHandle > -1) {
          return { class: 'autodown-row-resize-cursor' }
        }
        return { class: '' }
      },

      handleDOMEvents: {
        mousemove(view, event) {
          const pluginState = rowResizingPluginKey.getState(view.state) as PluginState | undefined
          if (!pluginState) return false

          // During drag
          if (pluginState.dragging) {
            const dy = event.clientY - pluginState.dragging.startY
            const newHeight = Math.max(minRowHeight, pluginState.dragging.startHeight + dy)

            const tr = view.state.tr
            const currentRow = view.state.doc.nodeAt(pluginState.dragging.rowPos)
            if (currentRow && currentRow.type.name === 'tableRow') {
              tr.setNodeMarkup(pluginState.dragging.rowPos, undefined, {
                ...currentRow.attrs,
                rowheight: Math.round(newHeight),
              })
              view.dispatch(tr)
            }
            return true
          }

          // Detect hover over row bottom boundary
          const hit = findRowAtPoint(view, event, 8)
          const newActive = hit ? hit.rowIndex : -1

          if (newActive !== pluginState.activeHandle) {
            view.dispatch(
              view.state.tr.setMeta(rowResizingPluginKey, { activeHandle: newActive })
            )
          }
          return false
        },

        mouseleave(view) {
          const pluginState = rowResizingPluginKey.getState(view.state) as PluginState | undefined
          if (pluginState && pluginState.activeHandle > -1 && !pluginState.dragging) {
            view.dispatch(
              view.state.tr.setMeta(rowResizingPluginKey, { activeHandle: -1 })
            )
          }
          return false
        },

        mousedown(view, event) {
          const pluginState = rowResizingPluginKey.getState(view.state) as PluginState | undefined
          if (!pluginState || pluginState.activeHandle === -1 || pluginState.dragging) return false

          const hit = findRowAtPoint(view, event, 8)
          if (!hit) return false

          const row = view.state.doc.nodeAt(hit.rowPos)
          if (!row || row.type.name !== 'tableRow') return false

          // Measure actual DOM height
          const dom = view.domAtPos(hit.rowPos + 1)
          const rowEl = dom.node instanceof HTMLElement ? dom.node : (dom.node as any).parentElement
          const startHeight = rowEl?.offsetHeight ?? (row.attrs.rowheight || minRowHeight)

          view.dispatch(
            view.state.tr.setMeta(rowResizingPluginKey, {
              dragging: { startY: event.clientY, startHeight, rowPos: hit.rowPos },
            })
          )

          const onMouseUp = () => {
            view.dispatch(
              view.state.tr.setMeta(rowResizingPluginKey, { dragging: null })
            )
            document.removeEventListener('mouseup', onMouseUp)
            document.removeEventListener('mousemove', onMouseMove)
          }

          const onMouseMove = (e: MouseEvent) => {
            const state = rowResizingPluginKey.getState(view.state) as PluginState | undefined
            if (!state?.dragging) return

            const dy = e.clientY - state.dragging.startY
            const newHeight = Math.max(minRowHeight, state.dragging.startHeight + dy)

            const tr = view.state.tr
            const currentRow = view.state.doc.nodeAt(state.dragging.rowPos)
            if (currentRow && currentRow.type.name === 'tableRow') {
              tr.setNodeMarkup(state.dragging.rowPos, undefined, {
                ...currentRow.attrs,
                rowheight: Math.round(newHeight),
              })
              view.dispatch(tr)
            }
          }

          document.addEventListener('mouseup', onMouseUp)
          document.addEventListener('mousemove', onMouseMove)

          event.preventDefault()
          return true
        },

        dblclick(view, event) {
          const hit = findRowAtPoint(view, event, 12)
          if (!hit) return false

          const row = view.state.doc.nodeAt(hit.rowPos)
          if (!row || row.type.name !== 'tableRow') return false

          // Double-click resets row height to auto
          const tr = view.state.tr
          tr.setNodeMarkup(hit.rowPos, undefined, {
            ...row.attrs,
            rowheight: null,
          })
          view.dispatch(tr)
          return true
        },
      },
    },
  })
}
