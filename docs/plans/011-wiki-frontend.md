# Plan: Wiki Frontend — View, Composable, Types, and Tab Integration

## Context

The wiki backend is complete (`backend/src/forge/wiki.rs`): CRUD API, agent tools, manifest storage, and REST routes. But there's no frontend to manage, view, or edit wiki pages. This plan covers P17.6 (Wiki tab with sidebar + markdown rendering) and P17.8 (markdown editor with preview).

The implementation follows the existing SpecsView pattern (sidebar list + content panel) and reuses `MarkdownContent.vue` and `MarkdownEditor.vue` already in the project.

---

## Step 1: Create wiki types

**New file:** `frontend/src/types/wiki.ts`

Match the backend data model from `wiki.rs`:
- `WikiSource` type: `'manual' | 'guide' | 'api_ref' | 'custom'`
- `WikiPage` interface: slug, title, content, source_type, tags, version, created_at, updated_at
- `WikiPageMeta` interface: slug, title, source_type, tags, version, updated_at (for list view)

---

## Step 2: Create useWiki composable

**New file:** `frontend/src/composables/useWiki.ts`

Follow the `useSpecs.ts` singleton pattern:
- Module-level refs: `_pages` (WikiPageMeta[]), `_activePage` (WikiPage | null), `_isLoading`, `_error`
- `loadPages(project)` — GET `/api/forge/wiki/{project}/pages`
- `loadPage(project, slug)` — GET `/api/forge/wiki/{project}/page/{slug}`
- `createPage(project, page)` — POST `/api/forge/wiki/{project}/pages`
- `updatePage(project, slug, data)` — PUT `/api/forge/wiki/{project}/page/{slug}`
- `deletePage(project, slug)` — DELETE `/api/forge/wiki/{project}/page/{slug}`
- `searchWiki(project, query)` — POST `/api/forge/wiki/{project}/search`

---

## Step 3: Create WikiView

**New file:** `frontend/src/views/WikiView.vue`

Layout matches SpecsView pattern:
```
.wiki-view (flex column)
└── .wiki-body (flex row)
    ├── .wiki-nav (220px sidebar, collapsible)
    │   ├── .wiki-nav-header (title + collapse btn + new page btn)
    │   ├── .wiki-nav-search (search input)
    │   └── .wiki-nav-item (page list — title, version, tags)
    └── .wiki-content (flex-1)
        ├── .wiki-content-header (page title, metadata, edit/delete buttons)
        ├── .wiki-content-body
        │   ├── MarkdownContent (view mode) OR MarkdownEditor (edit mode)
        │   └── Empty state if no page selected
        └── .wiki-content-footer (tags, version, updated_at)
```

Key behaviors:
- Sidebar lists pages from `useWiki().pages`, filtered by search
- Click page → load full content via `loadPage()`
- Edit button → switch to `MarkdownEditor` (split-pane markdown/preview)
- Save → `updatePage()`, then reload
- New page → inline slug/title form, then `createPage()`
- Delete → confirm dialog, then `deletePage()`
- Sidebar collapse persists to localStorage (key: `autoforge-wiki-sidebar-collapsed`)

Reuses existing components:
- `MarkdownContent.vue` — for rendered markdown view
- `MarkdownEditor.vue` — for edit mode (split-pane editor + preview)

---

## Step 4: Register Wiki tab in App.vue

**File:** `frontend/src/App.vue`

Changes:
1. Import `BookOpen` from `lucide-vue-next` (already available, used by SpecsView)
2. Import `WikiView` from `./views/WikiView.vue`
3. Add `wiki` to the `tabs` array (position: after Specs, before Relay): `{ id: 'wiki', label: 'Wiki', icon: BookOpen }`
4. Add `'wiki'` to the `currentView` type union
5. Add `<WikiView v-else-if="currentView === 'wiki'" />` in the template

---

## Files to create (2) / modify (1)

| File | Action |
|---|---|
| `frontend/src/types/wiki.ts` | Create — WikiPage, WikiPageMeta, WikiSource types |
| `frontend/src/composables/useWiki.ts` | Create — singleton state, API calls |
| `frontend/src/views/WikiView.vue` | Create — sidebar + content panel |
| `frontend/src/App.vue` | Modify — add wiki tab + import |

---

## Verification

1. `npm run build` in frontend — no compilation errors
2. Open the app, verify "Wiki" tab appears in navigation rail (BookOpen icon)
3. Click Wiki tab — verify empty state shows
4. Create a new page via the "+" button — fill slug, title, content, save
5. Verify page appears in sidebar, clicking it shows rendered markdown
6. Edit the page — verify MarkdownEditor opens, save updates content
7. Search in sidebar — verify page list filters
8. Delete page — verify it disappears from sidebar
