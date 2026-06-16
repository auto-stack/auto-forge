# Session Search Keyboard Shortcut Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Ctrl+Shift+S (Cmd+Shift+S on macOS) keyboard shortcut to focus the existing search input in `frontend/src/views/ChatsView.vue`.

**Architecture:** Extend the existing global `handleGlobalKeydown` listener with a new branch that focuses the search input via a Vue template ref.

**Tech Stack:** Vue 3 Composition API, TypeScript

---

### Task 1: Add Template Ref to Search Input

**Files:**
- Modify: `frontend/src/views/ChatsView.vue` (template, ~line 73)

- [x] **Step 1:** Locate the search input element (`<input v-model="chatSearch" ... />`) and add `ref="searchInputRef"`.

  ```vue
  <input
    ref="searchInputRef"
    v-model="chatSearch"
    type="text"
    class="search-input"
    :placeholder="t('chat.searchPlaceholder')"
  />
  ```

- [x] **Step 2:** Verify the attribute is present.

---

### Task 2: Declare the Ref

**Files:**
- Modify: `frontend/src/views/ChatsView.vue` (script setup, near other refs)

- [x] **Step 1:** Add the ref declaration:

  ```typescript
  const searchInputRef = ref<HTMLInputElement>()
  ```

- [x] **Step 2:** Verify TypeScript recognizes the ref.

---

### Task 3: Extend `handleGlobalKeydown`

**Files:**
- Modify: `frontend/src/views/ChatsView.vue` (script setup, `handleGlobalKeydown` function)

- [x] **Step 1:** Add the Ctrl+Shift+S branch after the existing Ctrl+Shift+N branch:

  ```typescript
  // Ctrl+Shift+S (or Cmd+Shift+S on macOS): Focus search input
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 's') {
    e.preventDefault()
    searchInputRef.value?.focus()
    return
  }
  ```

- [x] **Step 2:** Add `return` after the Ctrl+Shift+N branch to avoid both shortcuts firing on the same key event.

---

### Task 4: Verify Type Safety

**Files:**
- Check: `frontend/src/views/ChatsView.vue`

- [x] **Step 1:** Run `cd frontend && pnpm vue-tsc --noEmit`
- [x] **Step 2:** Expected: no TypeScript errors

---

### Task 5: Manual Testing

- [x] **Step 1:** Start the dev server (`cd frontend && pnpm dev`).
- [x] **Step 2:** Open ChatsView in the browser.
- [x] **Step 3:** Press Ctrl+Shift+S / Cmd+Shift+S and confirm the search input is focused.
- [x] **Step 4:** Type a query and confirm messages are filtered.
- [x] **Step 5:** Press Ctrl+Shift+N and confirm a new session is still created.

---

### Task 6: Commit

- [x] **Step 1:** Stage and commit the change:

  ```bash
  git add frontend/src/views/ChatsView.vue
  git commit -m "feat(chats): add Ctrl+Shift+S shortcut to focus search input"
  ```

---

## Summary

- Only `frontend/src/views/ChatsView.vue` is touched.
- Three small additions: template ref, ref declaration, and keyboard handler branch.
- No i18n changes are required because the shortcut is not surfaced in the UI as a label or tooltip.
