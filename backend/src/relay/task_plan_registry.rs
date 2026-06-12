//! TaskPlan registry — built-in + user-defined Atom plans.

use auto_atom::AtomResult;
use std::collections::HashMap;
use std::sync::Mutex;

use crate::relay::task_plan::TaskPlan;
use crate::relay::task_plan_parser::parse_task_plan;

// ─── Built-in TaskPlan Atoms ─────────────────────────────────────────────────

const BUILTIN_TASK_PLANS: &[(&str, &str)] = &[
    ("deferred-decompose", include_str!("task_plans/builtin/deferred-decompose.atom")),
];

/// Source of a TaskPlan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskPlanSource {
    Builtin,
    User,
}

/// Summary returned by `list()`.
#[derive(Debug, Clone)]
pub struct TaskPlanSummary {
    pub id: String,
    pub source: TaskPlanSource,
    pub phase_count: usize,
    pub run_count: usize,
}

/// Registry of all available TaskPlans (built-in + user-defined).
pub struct TaskPlanRegistry {
    plans: HashMap<String, (TaskPlan, TaskPlanSource)>,
}

impl TaskPlanRegistry {
    /// Create a new registry and load all plans.
    pub fn new(data_dir: &std::path::Path) -> Self {
        let mut registry = Self {
            plans: HashMap::new(),
        };
        registry.load_builtin();
        registry.load_from_dir(data_dir);
        registry
    }

    /// Load only built-in plans (useful for tests).
    pub fn load_builtins_only() -> Self {
        let mut registry = Self {
            plans: HashMap::new(),
        };
        registry.load_builtin();
        registry
    }

    /// Get a plan by ID.
    pub fn get(&self, plan_id: &str) -> Option<TaskPlan> {
        self.plans.get(plan_id).map(|(plan, _)| plan.clone())
    }

    /// Get the source of a plan.
    pub fn source(&self, plan_id: &str) -> Option<TaskPlanSource> {
        self.plans.get(plan_id).map(|(_, source)| *source)
    }

    /// List all available plans.
    pub fn list(&self) -> Vec<TaskPlanSummary> {
        self.plans
            .values()
            .map(|(plan, source)| TaskPlanSummary {
                id: plan.id.clone(),
                source: *source,
                phase_count: plan.phases.len(),
                run_count: plan.phases.iter().map(|p| p.runs.len()).sum(),
            })
            .collect()
    }

    /// Insert or overwrite a plan in the registry.
    pub fn insert(&mut self, plan: TaskPlan, source: TaskPlanSource) {
        self.plans.insert(plan.id.clone(), (plan, source));
    }

    /// Remove a plan. Built-in plans cannot be removed.
    pub fn remove(&mut self, plan_id: &str) -> Option<TaskPlan> {
        match self.plans.get(plan_id) {
            Some((_, TaskPlanSource::Builtin)) => None,
            Some(_) => self.plans.remove(plan_id).map(|(plan, _)| plan),
            None => None,
        }
    }

    /// Validate a plan against the registry. Currently checks structural
    /// validity. Future: also verify flow_ids against FlowRegistry.
    pub fn validate(&self, plan: &TaskPlan) -> AtomResult<()> {
        plan.validate()
    }

    fn load_builtin(&mut self) {
        for (_id, atom) in BUILTIN_TASK_PLANS {
            match parse_task_plan(atom) {
                Ok(plan) => {
                    if let Err(e) = self.validate(&plan) {
                        tracing::error!(
                            "Built-in TaskPlan '{}' validation error: {}",
                            plan.id, e
                        );
                        panic!(
                            "Built-in TaskPlan '{}' has validation errors",
                            plan.id
                        );
                    }
                    self.plans.insert(plan.id.clone(), (plan, TaskPlanSource::Builtin));
                }
                Err(e) => {
                    tracing::error!("Failed to parse built-in TaskPlan: {}", e);
                }
            }
        }
    }

    fn load_from_dir(&mut self, data_dir: &std::path::Path) {
        let plans_dir = data_dir.join(".autoforge").join("task_plans");
        if !plans_dir.is_dir() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(&plans_dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext != "atom" {
                continue;
            }
            match std::fs::read_to_string(&path) {
                Ok(content) => {
                    match parse_task_plan(&content) {
                        Ok(plan) => {
                            if let Err(e) = self.validate(&plan) {
                                tracing::error!(
                                    "User TaskPlan '{}' validation error: {} (from {:?})",
                                    plan.id, e, path
                                );
                            } else {
                                tracing::info!(
                                    "Loaded TaskPlan '{}' from {:?}",
                                    plan.id, path
                                );
                                self.plans.insert(plan.id.clone(), (plan, TaskPlanSource::User));
                            }
                        }
                        Err(e) => {
                            tracing::warn!("Failed to parse TaskPlan {:?}: {}", path, e);
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Failed to read TaskPlan {:?}: {}", path, e);
                }
            }
        }
    }
}

// ─── Global Registry ─────────────────────────────────────────────────────────

/// Lazy-initialized global TaskPlan registry.
pub(crate) static TASK_PLAN_REGISTRY: Mutex<Option<TaskPlanRegistry>> = Mutex::new(None);

/// Initialize the global TaskPlan registry from the current project path.
pub fn init_task_plan_registry() {
    if let Some(project_path) = crate::forge::current_project_path() {
        let path = std::path::PathBuf::from(project_path);
        let mut guard = TASK_PLAN_REGISTRY.lock().unwrap();
        *guard = Some(TaskPlanRegistry::new(&path));
    }
}

/// Get a TaskPlan from the global registry.
/// Auto-initializes on first call if a project is open.
pub fn get_task_plan(plan_id: &str) -> Option<TaskPlan> {
    {
        let guard = TASK_PLAN_REGISTRY.lock().unwrap();
        if let Some(ref registry) = *guard {
            return registry.get(plan_id);
        }
    }
    init_task_plan_registry();
    let guard = TASK_PLAN_REGISTRY.lock().unwrap();
    guard.as_ref()?.get(plan_id)
}

/// List all TaskPlans from the global registry.
pub fn list_task_plans() -> Vec<TaskPlanSummary> {
    {
        let guard = TASK_PLAN_REGISTRY.lock().unwrap();
        if let Some(ref registry) = *guard {
            return registry.list();
        }
    }
    init_task_plan_registry();
    let guard = TASK_PLAN_REGISTRY.lock().unwrap();
    guard.as_ref().map(|r| r.list()).unwrap_or_default()
}

/// Register a new TaskPlan from Atom source.
///
/// Validates the Atom, checks that all referenced flows exist, writes the file
/// to disk if `file_path` is provided, and inserts the plan into the global
/// registry.
pub fn register_task_plan(
    atom: &str,
    file_path: Option<&std::path::Path>,
) -> Result<TaskPlan, String> {
    let plan = parse_task_plan(atom).map_err(|e| e.to_string())?;
    plan.validate().map_err(|e| e.to_string())?;

    for phase in &plan.phases {
        for run in &phase.runs {
            if crate::relay::flows::get_flow(&run.flow_id).is_none() {
                return Err(format!(
                    "run '{}' references unknown flow '{}'",
                    run.name, run.flow_id
                ));
            }
        }
    }

    if let Some(path) = file_path {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, atom).map_err(|e| e.to_string())?;
    }

    {
        let mut guard = TASK_PLAN_REGISTRY.lock().unwrap();
        if guard.is_none() {
            *guard = Some(TaskPlanRegistry::load_builtins_only());
        }
        if let Some(ref mut registry) = *guard {
            registry.insert(plan.clone(), TaskPlanSource::User);
        }
    }

    Ok(plan)
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builtin_deferred_decompose_loads() {
        let registry = TaskPlanRegistry::load_builtins_only();
        let plan = registry.get("deferred-decompose");
        assert!(plan.is_some());
        let plan = plan.unwrap();
        assert_eq!(plan.phases.len(), 1);
        assert_eq!(plan.phases[0].runs.len(), 1);
    }

    #[test]
    fn test_cannot_remove_builtin() {
        let mut registry = TaskPlanRegistry::load_builtins_only();
        assert!(registry.remove("deferred-decompose").is_none());
        assert!(registry.get("deferred-decompose").is_some());
    }

    #[test]
    fn test_insert_and_remove_user_plan() {
        let mut registry = TaskPlanRegistry::load_builtins_only();
        let plan = TaskPlan::new("custom");
        registry.insert(plan, TaskPlanSource::User);
        assert!(registry.get("custom").is_some());
        assert!(registry.remove("custom").is_some());
        assert!(registry.get("custom").is_none());
    }

    #[test]
    fn test_list_plans() {
        let mut registry = TaskPlanRegistry::load_builtins_only();
        registry.insert(TaskPlan::new("custom"), TaskPlanSource::User);
        let list = registry.list();
        assert_eq!(list.len(), 2);
    }
}
