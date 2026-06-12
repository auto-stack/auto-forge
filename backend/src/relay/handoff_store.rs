//! Cross-run handoff storage for TaskPlan execution.
//!
//! When a relay run that belongs to a TaskPlan completes, its final handoff
//! is persisted here keyed by (task_plan_id, phase_name, run_name). Later
//! phases can reference it via `input_from: "phase.run.handoff.field"`.

use crate::relay::handoff::HandoffDocument;
use crate::relay::store::{get_run, RunStore};
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

/// Persist and query handoffs across TaskPlan runs.
#[derive(Debug)]
pub struct HandoffStore {
    project_path: PathBuf,
    /// Optional in-memory cache keyed by (task_plan_id, phase, run).
    cache: std::sync::Mutex<HashMap<(String, String, String), HandoffDocument>>,
}

impl HandoffStore {
    /// Create a store rooted in the project directory.
    pub fn new(project_path: impl Into<PathBuf>) -> Self {
        Self {
            project_path: project_path.into(),
            cache: std::sync::Mutex::new(HashMap::new()),
        }
    }

    /// Directory where handoffs are persisted.
    fn handoffs_dir(&self, task_plan_id: &str) -> PathBuf {
        self.project_path
            .join(".autoforge")
            .join("task_plans")
            .join(".handoffs")
            .join(task_plan_id)
    }

    /// File path for a specific handoff.
    fn handoff_path(&self, task_plan_id: &str, phase: &str, run: &str) -> PathBuf {
        self.handoffs_dir(task_plan_id)
            .join(phase)
            .join(format!("{}.json", run))
    }

    /// Save a handoff to disk and cache.
    pub fn save(
        &self,
        task_plan_id: &str,
        phase: &str,
        run: &str,
        handoff: &HandoffDocument,
    ) -> Result<(), String> {
        let path = self.handoff_path(task_plan_id, phase, run);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create handoff dir: {}", e))?;
        }
        let json = serde_json::to_string_pretty(handoff)
            .map_err(|e| format!("failed to serialize handoff: {}", e))?;
        std::fs::write(&path, json)
            .map_err(|e| format!("failed to write handoff {:?}: {}", path, e))?;

        self.cache
            .lock()
            .unwrap()
            .insert((task_plan_id.to_string(), phase.to_string(), run.to_string()), handoff.clone());
        Ok(())
    }

    /// Load a handoff from cache or disk.
    pub fn load(&self, task_plan_id: &str, phase: &str, run: &str) -> Option<HandoffDocument> {
        let key = (task_plan_id.to_string(), phase.to_string(), run.to_string());
        if let Some(doc) = self.cache.lock().unwrap().get(&key) {
            return Some(doc.clone());
        }
        let path = self.handoff_path(task_plan_id, phase, run);
        let content = std::fs::read_to_string(&path).ok()?;
        let doc: HandoffDocument = serde_json::from_str(&content).ok()?;
        self.cache.lock().unwrap().insert(key, doc.clone());
        Some(doc)
    }

    /// Resolve a path like `task_plan_id.phase.run.handoff.field` to a JSON value.
    ///
    /// Supported paths:
    /// - `task_plan_id.phase.run.handoff.summary`
    /// - `task_plan_id.phase.run.handoff.decisions`
    /// - `task_plan_id.phase.run.handoff.open_questions`
    /// - `task_plan_id.phase.run.handoff.spec_updates`
    /// - `task_plan_id.phase.run.handoff.work_product`
    /// - `task_plan_id.phase.run.handoff.context_for_next`
    /// - `task_plan_id.phase.run.handoff.token_usage`
    ///
    /// Returns `None` if the handoff or field does not exist.
    pub fn resolve_path(&self, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() < 5 || parts[3] != "handoff" {
            return None;
        }
        let task_plan_id = parts[0];
        let phase = parts[1];
        let run = parts[2];
        let handoff = self.load(task_plan_id, phase, run)?;

        let doc_json = serde_json::to_value(&handoff).ok()?;
        let mut value = &doc_json;
        for part in &parts[4..] {
            value = value.get(part)?;
        }
        Some(value.clone())
    }

    /// Resolve a path that includes the task_plan_id as the first segment:
    /// `task_plan_id.phase.run.handoff.field`.
    pub fn resolve_full_path(&self, path: &str) -> Option<Value> {
        let parts: Vec<&str> = path.split('.').collect();
        if parts.len() < 5 || parts[3] != "handoff" {
            return None;
        }
        let task_plan_id = parts[0];
        let phase = parts[1];
        let run = parts[2];
        let handoff = self.load(task_plan_id, phase, run)?;
        let doc_json = serde_json::to_value(&handoff).ok()?;
        let mut value = &doc_json;
        for part in &parts[4..] {
            value = value.get(part)?;
        }
        Some(value.clone())
    }

    /// Collect the final handoff from a completed relay run and save it.
    /// Returns the saved handoff if one was found.
    pub fn save_from_run(
        &self,
        store: &RunStore,
        task_plan_id: &str,
        phase: &str,
        run_name: &str,
        run_id: &str,
    ) -> Option<HandoffDocument> {
        let state = get_run(store, run_id)?;
        // The final handoff is in the last completed step record.
        let handoff = state
            .step_history
            .last()
            .and_then(|rec| rec.handoff.clone())?;
        self.save(task_plan_id, phase, run_name, &handoff)
            .map_err(|e| tracing::warn!("Failed to save handoff: {}", e))
            .ok()?;
        Some(handoff)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::relay::handoff::{HandoffDocument, TokenUsage};
    use tempfile::TempDir;

    #[test]
    fn save_and_load_handoff() {
        let dir = TempDir::new().unwrap();
        let store = HandoffStore::new(dir.path());
        let handoff = HandoffDocument::new("coder", "tester", "r1", 1);
        store.save("tp", "phase", "run", &handoff).unwrap();
        let loaded = store.load("tp", "phase", "run").unwrap();
        assert_eq!(loaded.from, "coder");
        assert_eq!(loaded.to, "tester");
    }

    #[test]
    fn resolve_full_path_summary() {
        let dir = TempDir::new().unwrap();
        let store = HandoffStore::new(dir.path());
        let mut handoff = HandoffDocument::new("coder", "tester", "r1", 1);
        handoff.summary = "Implemented auth".to_string();
        handoff.token_usage = TokenUsage {
            step_input: 100,
            step_output: 50,
            cumulative: 150,
            budget_remaining: 1000,
        };
        store.save("tp", "phase", "run", &handoff).unwrap();

        let summary = store.resolve_full_path("tp.phase.run.handoff.summary");
        assert_eq!(summary, Some(Value::String("Implemented auth".to_string())));

        let cumulative = store.resolve_full_path("tp.phase.run.handoff.token_usage.cumulative");
        assert_eq!(cumulative, Some(Value::Number(150.into())));
    }

    #[test]
    fn missing_handoff_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = HandoffStore::new(dir.path());
        assert!(store.load("tp", "phase", "run").is_none());
        assert!(store.resolve_full_path("tp.phase.run.handoff.summary").is_none());
    }
}
