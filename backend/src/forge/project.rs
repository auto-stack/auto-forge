use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    pub last_project_path: Option<String>,
    pub recent_projects: Vec<RecentProject>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecentProject {
    pub path: String,
    pub name: String,
    pub last_opened: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectInfo {
    pub path: String,
    pub name: String,
    pub specs_dir: String,
    pub has_specs: bool,
    pub is_open: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpenProjectRequest {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowseResponse {
    pub path: String,
    pub parent: Option<String>,
    pub children: Vec<BrowseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrowseEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectTreeNode {
    pub name: String,
    pub path: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub children: Option<Vec<ProjectTreeNode>>,
}

pub fn should_skip_entry(name: &str) -> bool {
    if name.starts_with('.') {
        return true;
    }
    const SKIP: &[&str] = &["node_modules", "target", "dist", "build", "__pycache__", "venv", ".venv"];
    SKIP.contains(&name)
}

pub fn build_project_tree(root: &std::path::Path) -> Vec<ProjectTreeNode> {
    let mut entries = Vec::new();
    let Ok(dir) = std::fs::read_dir(root) else {
        return entries;
    };
    let mut dir_entries: Vec<_> = dir.flatten().collect();
    dir_entries.sort_by(|a, b| {
        let a_is_dir = a.path().is_dir();
        let b_is_dir = b.path().is_dir();
        b_is_dir.cmp(&a_is_dir)
            .then(a.file_name().to_string_lossy().cmp(&b.file_name().to_string_lossy()))
    });
    for entry in &dir_entries {
        let name = entry.file_name().to_string_lossy().to_string();
        if should_skip_entry(&name) {
            continue;
        }
        let full_path = entry.path().to_string_lossy().to_string();
        if entry.path().is_dir() {
            let children = build_project_tree(&entry.path());
            entries.push(ProjectTreeNode {
                name,
                path: full_path,
                node_type: "folder".into(),
                children: Some(children),
            });
        } else {
            entries.push(ProjectTreeNode {
                name,
                path: full_path,
                node_type: "file".into(),
                children: None,
            });
        }
    }
    entries
}

pub fn read_project_file(path: &str) -> Result<String, String> {
    std::fs::read_to_string(path).map_err(|e| format!("Failed to read file: {}", e))
}

fn config_dir() -> PathBuf {
    dirs::data_local_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("autoforge")
}

fn config_path() -> PathBuf {
    config_dir().join("config.json")
}

pub fn load_config() -> ProjectConfig {
    let path = config_path();
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_config(config: &ProjectConfig) {
    let dir = config_dir();
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(config_path(), serde_json::to_string_pretty(config).unwrap_or_default());
}

pub fn add_recent(path: &str) {
    let mut config = load_config();
    let name = Path::new(path)
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Remove duplicate
    config.recent_projects.retain(|p| p.path != path);

    config.recent_projects.insert(0, RecentProject {
        path: path.to_string(),
        name,
        last_opened: now,
    });

    // Keep max 10
    config.recent_projects.truncate(10);
    config.last_project_path = Some(path.to_string());
    save_config(&config);
}

pub fn find_specs_dir(project_path: &Path) -> PathBuf {
    let new_loc = project_path.join("specs");
    let legacy_loc = project_path.join("docs").join("specs");

    if has_specs_content(&new_loc) {
        return new_loc;
    }
    if has_specs_content(&legacy_loc) {
        tracing::info!("Using legacy specs location: {}", legacy_loc.display());
        return legacy_loc;
    }
    // Neither exists — create new location
    let _ = std::fs::create_dir_all(&new_loc);
    new_loc
}

fn has_specs_content(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }
    // Has manifest.at or any .ad files
    if dir.join("manifest.at").exists() {
        return true;
    }
    std::fs::read_dir(dir).map_or(false, |mut d| {
        d.any(|e| e.map_or(false, |e| e.path().extension().map_or(false, |ext| ext == "ad")))
    })
}

pub fn browse_directory(path: &str) -> Result<BrowseResponse, String> {
    let dir = Path::new(path);
    if !dir.exists() {
        return Err(format!("Directory does not exist: {}", path));
    }
    if !dir.is_dir() {
        return Err(format!("Not a directory: {}", path));
    }

    let parent = dir.parent().map(|p| p.to_string_lossy().to_string());

    let mut children: Vec<BrowseEntry> = std::fs::read_dir(dir)
        .map_err(|e| format!("Cannot read directory: {}", e))?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            // Skip hidden directories
            !e.file_name()
                .to_string_lossy()
                .starts_with('.')
        })
        .map(|e| {
            let name = e.file_name().to_string_lossy().to_string();
            let full_path = e.path().to_string_lossy().to_string();
            BrowseEntry {
                name,
                path: full_path,
                is_dir: true,
            }
        })
        .collect();

    children.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));

    Ok(BrowseResponse {
        path: path.to_string(),
        parent,
        children,
    })
}
