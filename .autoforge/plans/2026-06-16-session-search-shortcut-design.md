# Session Search Keyboard Shortcut Design

## Goal
Add a keyboard shortcut (Ctrl+Shift+S / Cmd+Shift+S) in the Chats view to focus the existing search input, allowing users to quickly start searching without using the mouse.

## Context
The ChatsView.vue component already contains a search input in the header that filters messages within the current session. It currently has no keyboard access. The view also already has one global shortcut (`Ctrl+Shift+N` to create a new session) implemented in `handleGlobalKeydown`, registered in `onMounted` and removed in `onUnmounted`.

## Approaches Considered

| Approach | Pros | Cons |
|---|---|---|
| A: Add a template ref and call `.focus()` in `handleGlobalKeydown` | Follows existing pattern, type-safe, minimal change | Requires adding a ref to the input |
| B: Use `querySelector` to find the input by class | No template change | Brittle if class names change |
| C: Create a global keyboard shortcut composable | Reusable | Over-engineered for a single shortcut |

## Selected Approach
**Approach A** — add a template ref to the search input and focus it when Ctrl+Shift+S (or Cmd+Shift+S on macOS) is pressed. This follows the existing Ctrl+Shift+N pattern and requires only a few lines of code.

## Architecture / Data Flow

```
User presses Ctrl+Shift+S
  → window keydown listener invokes handleGlobalKeydown()
  → checks (ctrlKey || metaKey) && shiftKey && key.toLowerCase() === 's'
  → e.preventDefault() to avoid browser "Save As" dialog
  → searchInputRef.value?.focus()
  → search input receives focus and user can type immediately
```

## Files to Touch

- `frontend/src/views/ChatsView.vue`
  - Template: add `ref="searchInputRef"` to the search input
  - Script: declare `const searchInputRef = ref<HTMLInputElement>()`
  - Script: extend `handleGlobalKeydown()` with the Ctrl+Shift+S branch

## Testing Strategy

1. Open the Chats view.
2. Press Ctrl+Shift+S (Windows/Linux) or Cmd+Shift+S (macOS).
3. Verify the search input receives focus.
4. Type a query and verify messages are filtered.
5. Verify Ctrl+Shift+N still creates a new session.
6. Run `cd frontend && pnpm vue-tsc --noEmit` to confirm no type errors.

## Open Questions
None — the scope is intentionally minimal and follows established patterns.
