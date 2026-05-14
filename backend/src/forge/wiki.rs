//! Wiki Knowledge Layer — project-scoped knowledge base with agent tools.

use axum::{
    extract::Path,
    http::StatusCode,
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

// ─── Data Model ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPage {
    pub slug: String,
    pub title: String,
    pub content: String,
    pub source_type: WikiSource,
    pub tags: Vec<String>,
    pub version: u32,
    pub created_at: u64,
    pub updated_at: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WikiSource {
    Manual,
    Guide,
    ApiRef,
    Custom,
}

impl Default for WikiSource {
    fn default() -> Self {
        WikiSource::Custom
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct WikiManifest {
    pages: Vec<WikiPageMeta>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WikiPageMeta {
    pub slug: String,
    pub title: String,
    pub source_type: WikiSource,
    pub tags: Vec<String>,
    pub version: u32,
    pub updated_at: u64,
}

// ─── Wiki Store ──────────────────────────────────────────────────────────────

pub struct WikiStore {
    /// wiki_dir per project: project_name → wiki directory path
    wiki_dirs: HashMap<String, PathBuf>,
    /// Loaded pages: "project_name/slug" → WikiPage
    pages: HashMap<String, WikiPage>,
}

impl WikiStore {
    fn new() -> Self {
        Self {
            wiki_dirs: HashMap::new(),
            pages: HashMap::new(),
        }
    }

    fn wiki_dir(&self, project: &str) -> Option<&PathBuf> {
        self.wiki_dirs.get(project)
    }

    /// Ensure the wiki directory for a project exists, creating it if needed.
    fn ensure_dir(&mut self, project: &str, project_path: &str) -> PathBuf {
        let dir = self.wiki_dirs.get(project).cloned().unwrap_or_else(|| {
            std::path::Path::new(project_path).join("wiki")
        });
        let _ = std::fs::create_dir_all(&dir);
        self.wiki_dirs.insert(project.to_string(), dir.clone());
        dir
    }

    fn page_key(project: &str, slug: &str) -> String {
        format!("{}/{}", project, slug)
    }

    /// Load all wiki pages for a project from disk.
    pub fn load_project(&mut self, project: &str, project_path: &str) {
        let dir = self.ensure_dir(project, project_path);
        let manifest_path = dir.join("manifest.json");

        if let Ok(content) = std::fs::read_to_string(&manifest_path) {
            if let Ok(manifest) = serde_json::from_str::<WikiManifest>(&content) {
                for meta in &manifest.pages {
                    let page_path = dir.join(format!("{}.md", meta.slug));
                    if let Ok(page_content) = std::fs::read_to_string(&page_path) {
                        let page = WikiPage {
                            slug: meta.slug.clone(),
                            title: meta.title.clone(),
                            content: page_content,
                            source_type: meta.source_type.clone(),
                            tags: meta.tags.clone(),
                            version: meta.version,
                            created_at: 0,
                            updated_at: meta.updated_at,
                        };
                        self.pages.insert(Self::page_key(project, &meta.slug), page);
                    }
                }
            }
        }
    }

    /// List all pages for a project (metadata only, no content).
    pub fn list_pages(&self, project: &str) -> Vec<WikiPageMeta> {
        self.pages
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{}/", project)))
            .map(|(_, p)| WikiPageMeta {
                slug: p.slug.clone(),
                title: p.title.clone(),
                source_type: p.source_type.clone(),
                tags: p.tags.clone(),
                version: p.version,
                updated_at: p.updated_at,
            })
            .collect()
    }

    /// Get a single page by slug.
    pub fn get_page(&self, project: &str, slug: &str) -> Option<&WikiPage> {
        self.pages.get(&Self::page_key(project, slug))
    }

    /// Create a new wiki page.
    pub fn create_page(
        &mut self,
        project: &str,
        project_path: &str,
        page: WikiPage,
    ) -> Result<WikiPage, String> {
        let key = Self::page_key(project, &page.slug);
        if self.pages.contains_key(&key) {
            return Err(format!("Page '{}' already exists", page.slug));
        }

        let dir = self.ensure_dir(project, project_path);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let page = WikiPage {
            created_at: now,
            updated_at: now,
            version: 1,
            ..page
        };

        // Write markdown file
        let page_path = dir.join(format!("{}.md", page.slug));
        std::fs::write(&page_path, &page.content)
            .map_err(|e| format!("Failed to write page: {}", e))?;

        self.pages.insert(key, page.clone());
        self.save_manifest(project);

        Ok(page)
    }

    /// Update an existing wiki page.
    pub fn update_page(
        &mut self,
        project: &str,
        project_path: &str,
        slug: &str,
        content: String,
        title: Option<String>,
        tags: Option<Vec<String>>,
    ) -> Result<WikiPage, String> {
        let key = Self::page_key(project, slug);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let page = self
            .pages
            .get_mut(&key)
            .ok_or_else(|| format!("Page '{}' not found", slug))?;

        page.content = content;
        if let Some(t) = title {
            page.title = t;
        }
        if let Some(t) = tags {
            page.tags = t;
        }
        page.version += 1;
        page.updated_at = now;

        let updated = page.clone();

        let dir = self.ensure_dir(project, project_path);
        let page_path = dir.join(format!("{}.md", slug));
        std::fs::write(&page_path, &updated.content)
            .map_err(|e| format!("Failed to write page: {}", e))?;

        self.save_manifest(project);

        Ok(updated)
    }

    /// Delete a wiki page.
    pub fn delete_page(
        &mut self,
        project: &str,
        project_path: &str,
        slug: &str,
    ) -> Result<(), String> {
        let key = Self::page_key(project, slug);
        if self.pages.remove(&key).is_none() {
            return Err(format!("Page '{}' not found", slug));
        }

        let dir = self.ensure_dir(project, project_path);
        let page_path = dir.join(format!("{}.md", slug));
        if page_path.exists() {
            let _ = std::fs::remove_file(&page_path);
        }

        self.save_manifest(project);
        Ok(())
    }

    /// Search wiki pages by keyword (simple text search; RAG will replace this).
    pub fn search(&self, project: &str, query: &str) -> Vec<WikiPage> {
        let query_lower = query.to_lowercase();
        self.pages
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{}/", project)))
            .filter(|(_, p)| {
                let content_lower = p.content.to_lowercase();
                let title_lower = p.title.to_lowercase();
                content_lower.contains(&query_lower) || title_lower.contains(&query_lower)
            })
            .map(|(_, p)| p.clone())
            .collect()
    }

    fn save_manifest(&self, project: &str) {
        let Some(dir) = self.wiki_dirs.get(project) else { return };
        let metas: Vec<WikiPageMeta> = self
            .pages
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{}/", project)))
            .map(|(_, p)| WikiPageMeta {
                slug: p.slug.clone(),
                title: p.title.clone(),
                source_type: p.source_type.clone(),
                tags: p.tags.clone(),
                version: p.version,
                updated_at: p.updated_at,
            })
            .collect();

        let manifest = WikiManifest { pages: metas };
        if let Ok(json) = serde_json::to_string_pretty(&manifest) {
            let _ = std::fs::write(dir.join("manifest.json"), json);
        }
    }
}

// ─── Global Singleton ────────────────────────────────────────────────────────

pub fn wiki_store() -> &'static Mutex<WikiStore> {
    static STORE: OnceLock<Mutex<WikiStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(WikiStore::new()))
}

/// Load wiki pages for a project (call when project is opened or wiki tools first used).
pub fn ensure_wiki_loaded(project: &str, project_path: &str) {
    let mut store = wiki_store().lock().unwrap();
    if store.wiki_dir(project).is_none() {
        store.load_project(project, project_path);
    }
}

// ─── API DTOs ────────────────────────────────────────────────────────────────

#[derive(Serialize)]
struct WikiListResponse {
    pages: Vec<WikiPageMeta>,
}

#[derive(Serialize)]
struct WikiPageResponse {
    page: WikiPage,
}

#[derive(Deserialize)]
struct CreatePageRequest {
    slug: String,
    title: String,
    content: String,
    #[serde(default)]
    source_type: WikiSource,
    #[serde(default)]
    tags: Vec<String>,
}

#[derive(Deserialize)]
struct UpdatePageRequest {
    content: String,
    title: Option<String>,
    tags: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct SearchRequest {
    query: String,
}

#[derive(Serialize)]
struct SearchResponse {
    results: Vec<WikiPage>,
}

// ─── API Handlers ────────────────────────────────────────────────────────────

async fn list_wiki_pages(Path(project): Path<String>) -> Json<WikiListResponse> {
    let project_path = project_path_from_name(&project);
    ensure_wiki_loaded(&project, &project_path);
    let store = wiki_store().lock().unwrap();
    let pages = store.list_pages(&project);
    Json(WikiListResponse { pages })
}

async fn get_wiki_page(
    Path((project, slug)): Path<(String, String)>,
) -> Result<Json<WikiPageResponse>, StatusCode> {
    let project_path = project_path_from_name(&project);
    ensure_wiki_loaded(&project, &project_path);
    let store = wiki_store().lock().unwrap();
    match store.get_page(&project, &slug) {
        Some(page) => Ok(Json(WikiPageResponse { page: page.clone() })),
        None => Err(StatusCode::NOT_FOUND),
    }
}

async fn create_wiki_page_api(
    Path(project): Path<String>,
    Json(req): Json<CreatePageRequest>,
) -> Result<Json<WikiPageResponse>, (StatusCode, String)> {
    let project_path = project_path_from_name(&project);
    ensure_wiki_loaded(&project, &project_path);
    let mut store = wiki_store().lock().unwrap();
    let page = WikiPage {
        slug: req.slug,
        title: req.title,
        content: req.content,
        source_type: req.source_type,
        tags: req.tags,
        version: 0,
        created_at: 0,
        updated_at: 0,
    };
    store
        .create_page(&project, &project_path, page)
        .map(|p| Json(WikiPageResponse { page: p }))
        .map_err(|e| (StatusCode::CONFLICT, e))
}

async fn update_wiki_page_api(
    Path((project, slug)): Path<(String, String)>,
    Json(req): Json<UpdatePageRequest>,
) -> Result<Json<WikiPageResponse>, (StatusCode, String)> {
    let project_path = project_path_from_name(&project);
    ensure_wiki_loaded(&project, &project_path);
    let mut store = wiki_store().lock().unwrap();
    store
        .update_page(&project, &project_path, &slug, req.content, req.title, req.tags)
        .map(|p| Json(WikiPageResponse { page: p }))
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}

async fn delete_wiki_page_api(
    Path((project, slug)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    let project_path = project_path_from_name(&project);
    ensure_wiki_loaded(&project, &project_path);
    let mut store = wiki_store().lock().unwrap();
    store
        .delete_page(&project, &project_path, &slug)
        .map(|_| StatusCode::NO_CONTENT)
        .map_err(|e| (StatusCode::NOT_FOUND, e))
}

async fn search_wiki(
    Path(project): Path<String>,
    Json(req): Json<SearchRequest>,
) -> Json<SearchResponse> {
    let project_path = project_path_from_name(&project);
    ensure_wiki_loaded(&project, &project_path);
    let store = wiki_store().lock().unwrap();
    let results = store.search(&project, &req.query);
    Json(SearchResponse { results })
}

fn project_path_from_name(project: &str) -> String {
    // Try to resolve project name to path via the specs store
    let specs = crate::forge::specs().lock().unwrap();
    specs
        .data_dir
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("./{}", project))
}

// ─── Router ──────────────────────────────────────────────────────────────────

pub fn wiki_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        .route("/api/forge/wiki/{project}/pages", get(list_wiki_pages).post(create_wiki_page_api))
        .route("/api/forge/wiki/{project}/search", post(search_wiki))
        .route(
            "/api/forge/wiki/{project}/page/{slug}",
            get(get_wiki_page).put(update_wiki_page_api).delete(delete_wiki_page_api),
        )
}
