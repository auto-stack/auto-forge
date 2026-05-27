import TableRow from '@tiptap/extension-table-row'

/**
 * Extended TableRow with rowheight attribute for AutoDown table IAL.
 */
export const CustomTableRow = TableRow.extend({
  addAttributes() {
    return {
      ...this.parent?.(),
      rowheight: {
        default: null,
        parseHTML: (element) => {
          const rh = element.getAttribute('rowheight')
          if (!rh) return null
          const num = parseInt(rh, 10)
          return isNaN(num) ? null : num
        },
        renderHTML: (attributes) => {
          if (attributes.rowheight == null) return {}
          return { rowheight: String(attributes.rowheight) }
        },
      },
    }
  },
})
