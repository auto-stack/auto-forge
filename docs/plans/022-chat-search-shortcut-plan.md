# Chat Search Keyboard Shortcut Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: executing-plans. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Ctrl+Shift+S / Cmd+Shift+S keyboard shortcut to focus the chat message search input in ChatsView.vue.

**Architecture:** Add a template ref to the existing search input and extend the `handleGlobalKeydown()` function to focus the input when the shortcut is pressed.

**Tech Stack:** Vue 3 Composition API, TypeScript

---

### Task 1: Add searchInputRef Template Ref to Search Input

**Files:**
- Modify: `frontend/src/views/ChatsView.vue` (line 69-73)

- [ ] **Step 1: Add ref attribute to search input**
Find the search input element around line 69-73 and add `ref="searchInputRef"`:

```vue
<input
  ref="searchInputRef"
  v-model="chatSearch"
  type="text"
  class="search-input"
  :placeholder="t('chat.searchPlaceholder')"
/>
```

- [ ] **Step 2: Declare the ref in script setup**
Find the script setup section (around line 395) and add the ref declaration near other refs (look for `textareaRef` or `chatRef`):

```typescript
const searchInputRef = ref<HTMLInputElement | undefined>()
```

- [ ] **Step 3: Verify the changes**
Run: `cd frontend && npm run type-check`
Expected: No TypeScript errors

---

### Task 2: Extend handleGlobalKeydown to Handle Ctrl+Shift+S

**Files:**
- Modify: `frontend/src/views/ChatsView.vue` (lines 662-669)

- [ ] **Step 1: Add Ctrl+Shift+S handler to handleGlobalKeydown function**
Find the `handleGlobalKeydown` function (around line 662) and add the new shortcut handler inside the function, after the existing Ctrl+Shift+N handler:

```typescript
function handleGlobalKeydown(e: KeyboardEvent) {
  // Ctrl+Shift+N (or Cmd+Shift+N on macOS): Create new session
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 'n') {
    e.preventDefault()
    clearSession(projectPath?.value ?? undefined)
  }

  // Ctrl+Shift+S (or Cmd+Shift+S on macOS): Focus search input
  if ((e.ctrlKey || e.metaKey) && e.shiftKey && e.key.toLowerCase() === 's') {
    e.preventDefault()
    searchInputRef.value?.focus()
  }
}
```

- [ ] **Step 2: Verify the changes**
Run: `cd frontend && npm run type-check`
Expected: No TypeScript errors

---

### Task 3: Manual Testing

**Files:**
- Test: Manual browser testing

- [ ] **Step 1: Start the frontend dev server**
Run: `cd frontend && npm run dev`
Expected: Dev server starts successfully

- [ ] **Step 2: Open ChatsView in browser**
Navigate to the ChatsView in your browser at http://localhost:5173 (or the port shown)

- [ ] **Step 3: Test the keyboard shortcut**
Press Ctrl+Shift+S (on Windows/Linux) or Cmd+Shift+S (on macOS)
Expected: The search input in the header receives focus (cursor appears in the input)

- [ ] **Step 4: Test typing functionality**
With the search input focused, type a search query
Expected: Text appears in the search input and messages are filtered

- [ ] **Step 5: Test that existing shortcut still works**
Press Ctrl+Shift+N (or Cmd+Shift+N on macOS)
Expected: A new session is created (existing functionality still works)

- [ ] **Step 6: Test edge case - shortcut when already focused**
Press Ctrl+Shift+S when the search input is already focused
Expected: No side effects, input remains focused

- [ ] **Step 7: Test edge case - shortcut with collapsed sidebar**
Collapse the sidebar, then press Ctrl+Shift+S
Expected: Search input still receives focus correctly

---

### Task 4: Commit Changes

**Files:**
- Git commit

- [ ] **Step 1: Stage and commit the changes**
```bash
git add frontend/src/views/ChatsView.vue
git commit -m "feat: add Ctrl+Shift+S shortcut to focus chat search input

- Add searchInputRef template ref to search input
- Extend handleGlobalKeydown to handle Ctrl+Shift+S / Cmd+Shift+S
- Shortcut focuses the chat message search input in ChatsView
```

- [ ] **Step 2: Verify commit**
Run: `git log -1 --stat`
Expected: Commit shows changes to ChatsView.vue only

---

## Verification Checklist

After completing all tasks, verify:

- [ ] Search input has `ref="searchInputRef"` attribute
- [ ] Script setup has `const searchInputRef = ref<HTMLInputElement | undefined>()` declaration
- [ ] `handleGlobalKeydown` function includes Ctrl+Shift+S handler
- [ ] TypeScript compilation passes without errors
- [ ] Ctrl+Shift+S focuses search input in browser
- [ ] Ctrl+Shift+N still creates new session
- [ ] No other keyboard shortcuts are affected
- [ ] Changes committed to git

---

## Notes

- The feature is complete and requires no i18n string changes
- The implementation follows the existing pattern used for Ctrl+Shift+N
- Total changes: ~5 lines of code added to ChatsView.vue
