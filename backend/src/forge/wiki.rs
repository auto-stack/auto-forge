//! Wiki Knowledge Layer — multi-level directory with raw resource support.

use axum::{
    extract::{DefaultBodyLimit, Multipart, Path, Query},
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
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

// ─── Tree Node ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TreeNode {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub node_type: String, // "file" | "folder"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<TreeNode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub modified: Option<u64>,
}

fn build_tree(root: &std::path::Path, prefix: &str) -> Vec<TreeNode> {
    let mut entries: Vec<TreeNode> = Vec::new();
    let Ok(dir) = std::fs::read_dir(root) else {
        return entries;
    };
    let mut dir_entries: Vec<_> = dir.flatten().collect();
    dir_entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        b_is_dir
            .cmp(&a_is_dir)
            .then(a.file_name().to_string_lossy().cmp(&b.file_name().to_string_lossy()))
    });
    for entry in &dir_entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "_manifest.json" || name == "manifest.json" {
            continue;
        }
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        let meta = entry.metadata().ok();
        if entry.path().is_dir() {
            let children = build_tree(&entry.path(), &path);
            entries.push(TreeNode {
                name,
                path,
                node_type: "folder".into(),
                children: Some(children),
                size: None,
                modified: None,
            });
        } else {
            entries.push(TreeNode {
                name,
                path,
                node_type: "file".into(),
                children: None,
                size: meta.as_ref().map(|m| m.len()),
                modified: meta
                    .and_then(|m| m.modified().ok())
                    .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs()),
            });
        }
    }
    entries
}

// ─── Path Validation ─────────────────────────────────────────────────────────

fn validate_path(path: &str) -> Result<(), (StatusCode, String)> {
    if path.contains("..") || path.starts_with('/') || path.starts_with('\\') {
        return Err((StatusCode::BAD_REQUEST, "Invalid path".into()));
    }
    Ok(())
}

// ─── Wiki Store ──────────────────────────────────────────────────────────────

pub struct WikiStore {
    wiki_dirs: HashMap<String, PathBuf>,
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

    pub fn load_project(&mut self, project: &str, project_path: &str) {
        let dir = self.ensure_dir(project, project_path);

        // Load manifest metadata
        let metas: HashMap<String, WikiPageMeta> = std::fs::read_to_string(dir.join("_manifest.json"))
            .or_else(|_| std::fs::read_to_string(dir.join("manifest.json")))
            .ok()
            .and_then(|c| serde_json::from_str::<WikiManifest>(&c).ok())
            .map(|m| m.pages.into_iter().map(|p| (p.slug.clone(), p)).collect())
            .unwrap_or_default();

        // Walk wiki/ directory for all .md files
        let Ok(walker) = walk_md_files(&dir, "") else { return };
        for slug in walker {
            let page_path = dir.join(format!("{}.md", &slug));
            if let Ok(page_content) = std::fs::read_to_string(&page_path) {
                let meta = metas.get(&slug);
                let page = WikiPage {
                    slug: slug.clone(),
                    title: meta.map(|m| m.title.clone()).unwrap_or_else(|| slug.clone()),
                    content: page_content,
                    source_type: meta.map(|m| m.source_type.clone()).unwrap_or(WikiSource::Custom),
                    tags: meta.map(|m| m.tags.clone()).unwrap_or_default(),
                    version: meta.map(|m| m.version).unwrap_or(1),
                    created_at: 0,
                    updated_at: meta.map(|m| m.updated_at).unwrap_or(0),
                };
                self.pages.insert(Self::page_key(project, &slug), page);
            }
        }
    }

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

    pub fn get_page(&self, project: &str, slug: &str) -> Option<&WikiPage> {
        self.pages.get(&Self::page_key(project, slug))
    }

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

        let page_path = dir.join(format!("{}.md", page.slug));
        if let Some(parent) = page_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&page_path, &page.content)
            .map_err(|e| format!("Failed to write page: {}", e))?;

        self.pages.insert(key, page.clone());
        self.save_manifest(project);

        Ok(page)
    }

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
        if let Some(parent) = page_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::write(&page_path, &updated.content)
            .map_err(|e| format!("Failed to write page: {}", e))?;

        self.save_manifest(project);

        Ok(updated)
    }

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

    pub fn search(&self, project: &str, query: &str) -> Vec<WikiPage> {
        let query_lower = query.to_lowercase();
        self.pages
            .iter()
            .filter(|(k, _)| k.starts_with(&format!("{}/", project)))
            .filter(|(_, p)| {
                p.content.to_lowercase().contains(&query_lower)
                    || p.title.to_lowercase().contains(&query_lower)
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
            let _ = std::fs::write(dir.join("_manifest.json"), json);
        }
    }
}

fn walk_md_files(root: &std::path::Path, prefix: &str) -> Result<Vec<String>, String> {
    let mut result = Vec::new();
    let dir = std::fs::read_dir(root).map_err(|e| e.to_string())?;
    for entry in dir.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') || name == "_manifest.json" || name == "manifest.json" {
            continue;
        }
        let path = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{}/{}", prefix, name)
        };
        if entry.path().is_dir() {
            result.extend(walk_md_files(&entry.path(), &path)?);
        } else if name.ends_with(".md") {
            // Strip .md extension to get the slug
            result.push(path.trim_end_matches(".md").to_string());
        }
    }
    Ok(result)
}

// ─── Global Singleton ────────────────────────────────────────────────────────

pub fn wiki_store() -> &'static Mutex<WikiStore> {
    static STORE: OnceLock<Mutex<WikiStore>> = OnceLock::new();
    STORE.get_or_init(|| Mutex::new(WikiStore::new()))
}

pub fn ensure_wiki_loaded(project: &str, project_path: &str) {
    let mut store = wiki_store().lock().unwrap();
    if store.wiki_dir(project).is_none() {
        store.load_project(project, project_path);
    }
}

// ─── MIME Helper ─────────────────────────────────────────────────────────────

fn guess_mime(path: &std::path::Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()) {
        Some("md") => "text/markdown",
        Some("txt") => "text/plain",
        Some("pdf") => "application/pdf",
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("svg") => "image/svg+xml",
        Some("json") => "application/json",
        Some("csv") => "text/csv",
        Some("html") => "text/html",
        Some("js") => "application/javascript",
        Some("css") => "text/css",
        Some("xml") => "application/xml",
        Some("zip") => "application/zip",
        _ => "application/octet-stream",
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

#[derive(Deserialize)]
struct MkdirRequest {
    path: String,
}

#[derive(Deserialize)]
struct UploadQuery {
    #[serde(default)]
    prefix: String,
}

// ─── API Handlers ────────────────────────────────────────────────────────────

fn project_path_from_name(project: &str) -> String {
    let specs = crate::forge::specs().lock().unwrap();
    specs
        .data_dir
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| format!("./{}", project))
}

// Wiki tree
async fn wiki_tree(Path(project): Path<String>) -> Json<Vec<TreeNode>> {
    let project_path = project_path_from_name(&project);
    let wiki_dir = std::path::Path::new(&project_path).join("wiki");
    let mut tree = build_tree(&wiki_dir, "");
    // For wiki, strip .md extensions from file paths (wiki pages use slug without extension)
    strip_md_extensions(&mut tree);
    Json(tree)
}

fn strip_md_extensions(nodes: &mut [TreeNode]) {
    for node in nodes.iter_mut() {
        if node.node_type == "file" && node.name.ends_with(".md") {
            node.name = node.name.trim_end_matches(".md").to_string();
            node.path = node.path.trim_end_matches(".md").to_string();
        }
        if let Some(ref mut children) = node.children {
            strip_md_extensions(children);
        }
    }
}

// Raw tree
async fn raw_tree(Path(project): Path<String>) -> Json<Vec<TreeNode>> {
    let project_path = project_path_from_name(&project);
    let raw_dir = std::path::Path::new(&project_path).join("raw");
    let _ = std::fs::create_dir_all(&raw_dir);
    let tree = build_tree(&raw_dir, "");
    Json(tree)
}

// Wiki CRUD
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
    validate_path(&slug).map_err(|_| StatusCode::BAD_REQUEST)?;
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
    validate_path(&req.slug)?;
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
    validate_path(&slug)?;
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
    validate_path(&slug)?;
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

// Raw file upload
async fn raw_upload(
    Path(project): Path<String>,
    Query(query): Query<UploadQuery>,
    mut multipart: Multipart,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let project_path = project_path_from_name(&project);
    let raw_dir = std::path::Path::new(&project_path).join("raw");
    if !query.prefix.is_empty() {
        validate_path(&query.prefix)?;
    }

    let mut uploaded = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?
    {
        let filename = field.file_name().unwrap_or("unnamed").to_string();
        validate_path(&filename)?;
        let data = field
            .bytes()
            .await
            .map_err(|e| (StatusCode::BAD_REQUEST, e.to_string()))?;

        let target_dir = if query.prefix.is_empty() {
            raw_dir.clone()
        } else {
            raw_dir.join(&query.prefix)
        };
        let _ = std::fs::create_dir_all(&target_dir);
        let file_path = target_dir.join(&filename);

        std::fs::write(&file_path, &data)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let relative = if query.prefix.is_empty() {
            filename.clone()
        } else {
            format!("{}/{}", query.prefix, filename)
        };
        uploaded.push(relative);
    }
    Ok(Json(serde_json::json!({ "uploaded": uploaded })))
}

// Raw file serve
async fn raw_file(
    Path((project, path)): Path<(String, String)>,
) -> Result<Response, StatusCode> {
    validate_path(&path).map_err(|_| StatusCode::BAD_REQUEST)?;
    let project_path = project_path_from_name(&project);
    let raw_dir = std::path::Path::new(&project_path).join("raw");
    let file_path = raw_dir.join(&path);

    let data = std::fs::read(&file_path).map_err(|_| StatusCode::NOT_FOUND)?;
    let mime = guess_mime(&file_path);

    Ok(([(header::CONTENT_TYPE, mime)], data).into_response())
}

// Raw file/folder delete
async fn raw_delete(
    Path((project, path)): Path<(String, String)>,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_path(&path)?;
    let project_path = project_path_from_name(&project);
    let raw_dir = std::path::Path::new(&project_path).join("raw");
    let file_path = raw_dir.join(&path);
    if !file_path.exists() {
        return Err((StatusCode::NOT_FOUND, "Not found".into()));
    }
    if file_path.is_dir() {
        std::fs::remove_dir_all(&file_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    } else {
        std::fs::remove_file(&file_path)
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    }
    Ok(StatusCode::NO_CONTENT)
}

// Raw mkdir
async fn raw_mkdir(
    Path(project): Path<String>,
    Json(req): Json<MkdirRequest>,
) -> Result<StatusCode, (StatusCode, String)> {
    validate_path(&req.path)?;
    let project_path = project_path_from_name(&project);
    let raw_dir = std::path::Path::new(&project_path).join("raw");
    let target = raw_dir.join(&req.path);
    std::fs::create_dir_all(&target)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

// ─── Router ──────────────────────────────────────────────────────────────────

pub fn wiki_routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    Router::new()
        // Wiki tree + CRUD
        .route("/api/forge/wiki/{project}/tree", get(wiki_tree))
        .route(
            "/api/forge/wiki/{project}/pages",
            get(list_wiki_pages).post(create_wiki_page_api),
        )
        .route("/api/forge/wiki/{project}/search", post(search_wiki))
        .route(
            "/api/forge/wiki/{project}/page/{*slug}",
            get(get_wiki_page)
                .put(update_wiki_page_api)
                .delete(delete_wiki_page_api),
        )
        // Raw tree + CRUD
        .route("/api/forge/raw/{project}/tree", get(raw_tree))
        .route(
            "/api/forge/raw/{project}/upload",
            post(raw_upload).layer(DefaultBodyLimit::max(50 * 1024 * 1024)),
        )
        .route("/api/forge/raw/{project}/mkdir", post(raw_mkdir))
        .route(
            "/api/forge/raw/{project}/file/{*path}",
            get(raw_file).delete(raw_delete),
        )
}
