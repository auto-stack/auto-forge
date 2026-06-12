import { describe, it, expect, beforeEach, vi } from 'vitest'
import { mount } from '@vue/test-utils'
import { createI18n } from 'vue-i18n'
import { h } from 'vue'
import ConfirmDeleteDialog from '../ConfirmDeleteDialog.vue'

// ─── i18n setup ────────────────────────────────────────────────────────────

const i18n = createI18n({
  legacy: false,
  locale: 'en',
  messages: {
    en: {
      chat: {
        deleteAllSessions: 'Delete all sessions',
        confirmDeleteAll: 'Delete ALL sessions? All messages and memory will be lost. This cannot be undone.',
        cancel: 'Cancel',
        confirmDelete: 'Confirm Delete',
      },
    },
  },
})

// ─── Helper ────────────────────────────────────────────────────────────────
// ConfirmDeleteDialog uses <Teleport to="body">. In JSDOM, teleport content
// is appended to document.body rather than the wrapper's own DOM subtree.
// We attach to the document element so we can find the teleported content.

function mountDialog(props: { open: boolean; loading?: boolean }) {
  return mount(ConfirmDeleteDialog, {
    props,
    global: {
      plugins: [i18n],
    },
    attachTo: document.body,
  })
}

// Find the overlay inside the real document body (where Teleport renders)
function findOverlay() {
  return document.body.querySelector('.confirm-delete-overlay') as HTMLElement | null
}

// ─── Tests ─────────────────────────────────────────────────────────────────

describe('ConfirmDeleteDialog', () => {
  beforeEach(() => {
    vi.clearAllMocks()
    // Clean up any leftover teleported DOM between tests
    document.body.innerHTML = ''
  })

  // TC-T22.9: Dialog renders when open=true
  it('TC-T22.9: renders dialog when open is true', () => {
    mountDialog({ open: true })

    const overlay = document.body.querySelector('.confirm-delete-overlay') as HTMLElement
    expect(overlay).toBeTruthy()
    expect(overlay.getAttribute('role')).toBe('alertdialog')
    expect(overlay.getAttribute('aria-modal')).toBe('true')
  })

  // TC-T22.10: Dialog hidden when open=false
  it('TC-T22.10: does not render dialog when open is false', () => {
    mountDialog({ open: false })

    const overlay = document.body.querySelector('.confirm-delete-overlay')
    expect(overlay).toBeNull()
  })

  // TC-T22.11: Cancel button emits cancel event
  it('TC-T22.11: cancel button emits cancel event', async () => {
    const wrapper = mountDialog({ open: true })

    const cancelBtn = document.body.querySelector('.confirm-delete-cancel') as HTMLButtonElement
    expect(cancelBtn).toBeTruthy()
    cancelBtn.click()
    await vi.dynamicImportSettled()

    expect(wrapper.emitted('cancel')).toBeTruthy()
    expect(wrapper.emitted('cancel')).toHaveLength(1)
  })

  // TC-T22.12: Confirm button emits confirm event
  it('TC-T22.12: confirm button emits confirm event', async () => {
    const wrapper = mountDialog({ open: true })

    const confirmBtn = document.body.querySelector('.confirm-delete-confirm') as HTMLButtonElement
    expect(confirmBtn).toBeTruthy()
    confirmBtn.click()
    await vi.dynamicImportSettled()

    expect(wrapper.emitted('confirm')).toBeTruthy()
    expect(wrapper.emitted('confirm')).toHaveLength(1)
  })

  // TC-T22.13: Buttons disabled when loading
  it('TC-T22.13: buttons are disabled when loading is true', () => {
    mountDialog({ open: true, loading: true })

    const cancelBtn = document.body.querySelector('.confirm-delete-cancel') as HTMLButtonElement
    const confirmBtn = document.body.querySelector('.confirm-delete-confirm') as HTMLButtonElement

    expect(cancelBtn.disabled).toBe(true)
    expect(confirmBtn.disabled).toBe(true)
  })

  // TC-T22.14: Spinner shown when loading
  it('TC-T22.14: spinner shown when loading is true', () => {
    mountDialog({ open: true, loading: true })

    const spinner = document.body.querySelector('.confirm-delete-spinner')
    expect(spinner).toBeTruthy()

    // Confirm text should NOT be shown (spinner replaces it)
    const confirmBtn = document.body.querySelector('.confirm-delete-confirm') as HTMLButtonElement
    expect(confirmBtn.textContent).not.toContain('Confirm Delete')
  })

  // TC-T22.15: Escape key emits cancel
  it('TC-T22.15: escape key emits cancel event', async () => {
    const wrapper = mountDialog({ open: true })

    const overlay = document.body.querySelector('.confirm-delete-overlay') as HTMLElement
    expect(overlay).toBeTruthy()
    overlay.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape' }))
    await vi.dynamicImportSettled()

    expect(wrapper.emitted('cancel')).toBeTruthy()
  })

  // TC-T22.16: Dialog has correct aria-label
  it('TC-T22.16: dialog has correct aria-label from i18n', () => {
    mountDialog({ open: true })

    const overlay = document.body.querySelector('.confirm-delete-overlay') as HTMLElement
    expect(overlay.getAttribute('aria-label')).toBe('Delete all sessions')
  })

  // TC-T22.17: Confirm button has destructive (red) style class
  it('TC-T22.17: confirm button has destructive styling class', () => {
    mountDialog({ open: true })

    const confirmBtn = document.body.querySelector('.confirm-delete-confirm')
    expect(confirmBtn).toBeTruthy()
    expect(confirmBtn!.classList.contains('confirm-delete-confirm')).toBe(true)
  })
})
