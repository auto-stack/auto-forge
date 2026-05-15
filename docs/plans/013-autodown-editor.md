# 013 — AutoDown Editor (WYSIWYG Block-Based)

> **Status:** Draft  
> **Depends on:** [AutoDown Editor Design](../../docs/design/autodown-editor.md)  
> **Goal:** G19, G19.1, G19.2, G19.3  
> **Design:** D27  
> **Architecture:** A11

---

## 1. Context

The current editor is a split-pane textarea + preview (`MarkdownEditor.vue`). Users type raw Markdown on the left and see a rendered preview on the right. This is adequate for simple edits but lacks the structured, interactive editing experience expected for a modern knowledge base and spec system.

This plan replaces the split-pane editor with a **WYSIWYG block-based editor** built on Tiptap and inspired by Novel.sh. The editor treats every paragraph, heading, list, code block, and custom element as an editable block with slash commands, drag handles, and bubble menus.

---

## 2. Objective

Deliver a three-phase implementation:

1. **Phase 1** — Markdown WYSIWYG: Drop-in replacement for `MarkdownEditor.vue` with full CommonMark/GFM support.
2. **Phase 2** — AutoDown Extensions: Custom blocks (callouts, math, mermaid, spec references) with Vue component rendering.
3. **Phase 3** — Advanced: Collaboration, AI assist, and version diff (deferred).

---

## 3. Risk & Mitigation

**Risk:** Tiptap's `@tiptap/markdown` extension may not perfectly round-trip all GFM edge cases (tables with alignment, nested blockquotes, etc.).
**Mitigation:** Extensive round-trip tests in Phase 1. Fallback to ProseMirror JSON for clipboard/internal state; Markdown is only for persistence.

**Risk:** Vue node views (custom blocks) may cause performance issues with many blocks on large documents.
**Mitigation:** Virtual scrolling for documents >200 blocks. Lazy-load heavy renderers (Mermaid, KaTeX) only when blocks enter viewport.

**Risk:** Bundle size increase from Tiptap + extensions.
**Mitigation:** Code-split the editor into a separate chunk. Lazy-load `AutoDownEditor.vue` only when entering edit mode.

---

## 4. Phase Breakdown

### Phase 1 — Markdown WYSIWYG Core (Week 1)

| Task | Duration | Owner | Deliverable |
|---|---|---|---|
| Add Tiptap dependencies to frontend | 1h | UI | `package.json` updated, lockfile synced |
| Scaffold `components/editors/autodown/` directory structure | 1h | UI | Folders for extensions, blocks, menus, composables, styles |
| Create `createExtensions()` factory with StarterKit + Markdown | 2h | UI | `extensions/index.ts` exports working extension array |
| Build `AutoDownEditor.vue` core wrapper around `useEditor` | 2h | UI | Editor mounts, accepts `content` prop, emits `update` with markdown |
| Implement `EditorContent.vue` — ProseMirror view mount | 1h | UI | Thin wrapper, styled with `--af-*` CSS variables |
| Implement slash command extension + `SlashMenu.vue` | 4h | UI | `/` opens menu, filters items, keyboard navigation, inserts blocks |
| Implement bubble menu extension + `BubbleMenu.vue` | 3h | UI | Appears on text selection, bold/italic/code/link buttons |
| Integrate drag handle for block reordering | 2h | UI | Handle visible on hover, drag reorders blocks |
| Style all blocks to match AutoForge design system | 3h | UI | Uses `--af-card`, `--af-border`, `--af-fg`, monospace for code |
| Replace `MarkdownEditor.vue` usage in WikiView | 1h | UI | Wiki edit mode uses new editor |
| Replace `MarkdownEditor.vue` usage in Spec detail views | 1h | UI | Spec item edit mode uses new editor |
| Write round-trip tests: Markdown → Editor → Markdown | 3h | UI | Vitest tests verify no data loss for GFM features |

### Phase 2 — AutoDown Extensions (Week 2)

| Task | Duration | Owner | Deliverable |
|---|---|---|---|
| Design `BlockExtension` interface: Node + Vue component + markdown hooks | 2h | UI | Typed interface in `types/autodown.ts` |
| Implement Callout block: `:::type` syntax + `CalloutBlock.vue` | 3h | UI | Custom tokenizer, Vue rendering, type switcher |
| Implement Math block: `%{expr}%` syntax + `MathBlock.vue` with KaTeX | 3h | UI | Lazy-loaded KaTeX, inline display |
| Implement Mermaid block: `` ```mermaid `` + `MermaidBlock.vue` | 3h | UI | Uses existing `mermaid` dependency |
| Implement SpecRef block: `[[G1]]` syntax + `SpecRefBlock.vue` | 2h | UI | Clickable chip, links to spec detail |
| Implement CodeBlock with language selector | 2h | UI | Replaces StarterKit codeBlock, adds language attr |
| Implement ImageBlock with resize handles | 3h | UI | Width/height attrs, drag-to-resize |
| Add all AutoDown extensions to `createExtensions()` | 1h | UI | Registry includes all Phase 2 blocks |
| Write extension-specific parse/serialize tests | 2h | UI | Each block round-trips correctly |

### Phase 3 — Advanced Features (Deferred)

| Task | Duration | Owner | Deliverable |
|---|---|---|---|
| Multi-column layout blocks | 3d | UI | Side-by-side content blocks |
| Yjs real-time collaboration | 5d | Core | Multi-user cursors, concurrent editing |
| AI inline completion | 3d | Core | Ghost text suggestion, accept/reject |
| Version diff view | 2d | UI | Side-by-side ProseMirror document diff |

---

## 5. Dependencies

### New npm packages

```json
{
  "@tiptap/core": "^2.11.0",
  "@tiptap/vue-3": "^2.11.0",
  "@tiptap/starter-kit": "^2.11.0",
  "@tiptap/extension-placeholder": "^2.11.0",
  "@tiptap/extension-image": "^2.11.0",
  "@tiptap/extension-link": "^2.11.0",
  "@tiptap/extension-task-list": "^2.11.0",
  "@tiptap/extension-task-item": "^2.11.0",
  "@tiptap/extension-markdown": "^2.11.0",
  "@tiptap/extension-table": "^2.11.0",
  "@tiptap/extension-table-row": "^2.11.0",
  "@tiptap/extension-table-cell": "^2.11.0",
  "@tiptap/extension-table-header": "^2.11.0",
  "@tiptap/suggestion": "^2.11.0",
  "@tiptap/extension-drag-handle": "^2.11.0",
  "tiptap-markdown": "^0.8.10"
}
```

### Existing packages reused
- `mermaid` (already in `package.json`) — for MermaidBlock rendering
- `marked` (already in `package.json`) — Tiptap Markdown uses its own `marked` instance, but we keep ours for non-editor rendering
- `lucide-vue-next` (already in `package.json`) — icons for slash menu, bubble menu, drag handle

---

## 6. Acceptance Criteria

### Phase 1 Done When
- [ ] Typing `/` opens a menu with block types (Heading, List, Code, Quote, etc.)
- [ ] Selecting text shows a bubble menu with Bold, Italic, Link, Code
- [ ] Hovering a block shows a drag handle; dragging reorders the block
- [ ] Saving a Wiki page produces the same Markdown as the old editor (±whitespace)
- [ ] All existing `MarkdownEditor.vue` usages are replaced
- [ ] Editor is styled with AutoForge CSS variables (dark mode compatible)

### Phase 2 Done When
- [ ] `:::info` fences render as styled callout blocks with icons
- [ ] `%{math}%` renders as KaTeX-formatted math
- [ ] `` ```mermaid `` blocks render as live diagrams
- [ ] `[[G1]]` renders as clickable spec reference chips
- [ ] Each custom block serializes back to its Markdown syntax on save
- [ ] Adding a new block type requires only one `.ts` extension + one `.vue` component

---

## 7. Rollback Plan

If critical bugs are found in production:
1. Revert the component substitution in `WikiView.vue` and spec detail views.
2. Keep the `autodown/` directory in source control but do not import it.
3. The old `MarkdownEditor.vue` remains untouched and can be restored immediately.

---

## 8. References

- [AutoDown Editor Design](../../docs/design/autodown-editor.md) — full architecture and component design
- [Tiptap Documentation](https://tiptap.dev/) — extension API, node views, markdown
- [Novel.sh Source](https://github.com/steven-tey/novel) — reference implementation patterns
