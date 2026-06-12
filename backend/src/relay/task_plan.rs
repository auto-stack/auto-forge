//! TaskPlan data model for multi-relay orchestration.
//!
//! A TaskPlan is a tree of phases and runs authored in static Atom.
//! It is the macro layer above a single YAML `FlowSpec`.

use auto_atom::{AtomError, AtomResult};
use auto_val::{Kid, Node, Value};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// A multi-relay task plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskPlan {
    pub id: String,
    pub version: u32,
    pub title: Option<String>,
    pub description: Option<String>,
    pub default_mode: TaskMode,
    pub phases: Vec<Phase>,
}

impl TaskPlan {
    /// Create a new empty TaskPlan.
    pub fn new(id: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            version: 1,
            title: None,
            description: None,
            default_mode: TaskMode::default(),
            phases: Vec::new(),
        }
    }

    /// Add a phase to the plan.
    pub fn add_phase(mut self, phase: Phase) -> Self {
        self.phases.push(phase);
        self
    }

    /// Validate the plan structure.
    pub fn validate(&self) -> AtomResult<()> {
        let mut phase_names = HashSet::new();
        for phase in &self.phases {
            if !phase_names.insert(phase.name.clone()) {
                return Err(AtomError::ValidationError(format!(
                    "duplicate phase name '{}'",
                    phase.name
                )));
            }
        }

        for phase in &self.phases {
            let mut run_names = HashSet::new();
            for run in &phase.runs {
                if !run_names.insert(run.name.clone()) {
                    return Err(AtomError::ValidationError(format!(
                        "duplicate run name '{}' in phase '{}'",
                        run.name, phase.name
                    )));
                }
            }

            for dep in &phase.depends_on {
                if !phase_names.contains(dep) {
                    return Err(AtomError::ValidationError(format!(
                        "phase '{}' depends on unknown phase '{}'",
                        phase.name, dep
                    )));
                }
            }
        }

        // Check for dependency cycles.
        self.detect_cycle()?;

        // Validate input_from path syntax.
        for phase in &self.phases {
            for run in &phase.runs {
                for path in &run.input_from {
                    validate_handoff_path(path)?;
                }
            }
        }

        Ok(())
    }

    fn detect_cycle(&self) -> AtomResult<()> {
        let mut visited = HashSet::new();
        let mut stack = HashSet::new();
        let graph: HashMap<String, Vec<String>> = self
            .phases
            .iter()
            .map(|p| (p.name.clone(), p.depends_on.clone()))
            .collect();

        for phase in &self.phases {
            if !visited.contains(&phase.name) {
                if Self::dfs(&phase.name, &graph, &mut visited, &mut stack) {
                    return Err(AtomError::ValidationError(
                        "dependency cycle detected between phases".to_string(),
                    ));
                }
            }
        }
        Ok(())
    }

    fn dfs(
        node: &str,
        graph: &HashMap<String, Vec<String>>,
        visited: &mut HashSet<String>,
        stack: &mut HashSet<String>,
    ) -> bool {
        if stack.contains(node) {
            return true;
        }
        if visited.contains(node) {
            return false;
        }
        visited.insert(node.to_string());
        stack.insert(node.to_string());

        if let Some(deps) = graph.get(node) {
            for dep in deps {
                if Self::dfs(dep, graph, visited, stack) {
                    return true;
                }
            }
        }

        stack.remove(node);
        false
    }
}

/// Execution mode for a TaskPlan or individual run.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum TaskMode {
    /// Get Shit Done — autonomous execution.
    #[default]
    Gsd,
    /// Human reviews gates.
    Check,
}

/// A phase is a group of runs with a shared execution mode.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Phase {
    pub name: String,
    pub mode: PhaseMode,
    pub depends_on: Vec<String>,
    pub runs: Vec<RunRef>,
}

impl Phase {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            mode: PhaseMode::Serial,
            depends_on: Vec::new(),
            runs: Vec::new(),
        }
    }

    pub fn with_mode(mut self, mode: PhaseMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn depends_on(mut self, names: Vec<String>) -> Self {
        self.depends_on = names;
        self
    }

    pub fn add_run(mut self, run: RunRef) -> Self {
        self.runs.push(run);
        self
    }
}

/// Execution mode within a phase.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum PhaseMode {
    /// Run one after another.
    #[default]
    Serial,
    /// Run concurrently and join before continuing.
    Parallel,
}

/// A reference to a single relay run inside a phase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunRef {
    pub name: String,
    pub flow_id: String,
    pub input: Option<String>,
    pub input_from: Vec<String>,
    pub context: Option<String>,
    pub mode_override: Option<TaskMode>,
}

impl RunRef {
    pub fn new(name: impl Into<String>, flow_id: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            flow_id: flow_id.into(),
            input: None,
            input_from: Vec::new(),
            context: None,
            mode_override: None,
        }
    }

    pub fn with_input(mut self, input: impl Into<String>) -> Self {
        self.input = Some(input.into());
        self
    }

    pub fn with_input_from(mut self, paths: Vec<String>) -> Self {
        self.input_from = paths;
        self
    }

    pub fn with_context(mut self, context: impl Into<String>) -> Self {
        self.context = Some(context.into());
        self
    }

    pub fn with_mode_override(mut self, mode: TaskMode) -> Self {
        self.mode_override = Some(mode);
        self
    }
}

/// Convert a parsed Atom node into a `TaskPlan`.
impl TryFrom<Node> for TaskPlan {
    type Error = AtomError;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        if node.name != "task_plan" {
            return Err(AtomError::ValidationError(format!(
                "expected root node 'task_plan', found '{}'",
                node.name
            )));
        }

        let id = require_string_prop(&node, "id")?;
        let version = prop(&node, "version")
            .map(value_as_u32)
            .transpose()?
            .unwrap_or(1);
        let title = prop(&node, "title").map(value_as_string).transpose()?;
        let description = prop(&node, "description")
            .map(value_as_string)
            .transpose()?;
        let default_mode = prop(&node, "default_mode")
            .map(value_as_task_mode)
            .transpose()?
            .unwrap_or_default();

        let mut phases = Vec::new();
        for (_key, kid) in node.kids_iter() {
            if let Kid::Node(child) = kid {
                if child.name == "phase" {
                    phases.push(Phase::try_from(child.as_ref().clone())?);
                }
            }
        }

        let plan = TaskPlan {
            id,
            version,
            title,
            description,
            default_mode,
            phases,
        };
        plan.validate()?;
        Ok(plan)
    }
}

impl TryFrom<Node> for Phase {
    type Error = AtomError;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        if node.name != "phase" {
            return Err(AtomError::ValidationError(format!(
                "expected node 'phase', found '{}'",
                node.name
            )));
        }

        let name = require_string_prop(&node, "name")?;
        let mode = prop(&node, "mode")
            .map(value_as_phase_mode)
            .transpose()?
            .unwrap_or_default();
        let depends_on = prop(&node, "depends_on")
            .map(value_as_string_list)
            .transpose()?
            .unwrap_or_default();

        let mut runs = Vec::new();
        for (_key, kid) in node.kids_iter() {
            if let Kid::Node(child) = kid {
                if child.name == "run" {
                    runs.push(RunRef::try_from(child.as_ref().clone())?);
                }
            }
        }

        Ok(Phase {
            name,
            mode,
            depends_on,
            runs,
        })
    }
}

impl TryFrom<Node> for RunRef {
    type Error = AtomError;

    fn try_from(node: Node) -> Result<Self, Self::Error> {
        if node.name != "run" {
            return Err(AtomError::ValidationError(format!(
                "expected node 'run', found '{}'",
                node.name
            )));
        }

        let name = require_string_prop(&node, "name")?;
        let flow_id = require_string_prop(&node, "flow_id")?;
        let input = prop(&node, "input").map(value_as_string).transpose()?;
        let input_from = prop(&node, "input_from")
            .map(value_as_string_list)
            .transpose()?
            .unwrap_or_default();
        let context = prop(&node, "context").map(value_as_string).transpose()?;
        let mode_override = prop(&node, "mode_override")
            .map(value_as_task_mode)
            .transpose()?;

        Ok(RunRef {
            name,
            flow_id,
            input,
            input_from,
            context,
            mode_override,
        })
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn prop(node: &Node, key: &str) -> Option<Value> {
    match node.get_prop_of(key) {
        Value::Nil => None,
        value => Some(value),
    }
}

fn require_string_prop(node: &Node, key: &str) -> AtomResult<String> {
    prop(node, key)
        .ok_or_else(|| AtomError::MissingField(key.to_string()))
        .and_then(value_as_string)
}

fn value_as_string(value: Value) -> AtomResult<String> {
    match value {
        Value::Str(s) => Ok(s.to_string()),
        Value::Node(n) => Ok(n.id().to_string()),
        _ => Err(AtomError::InvalidType {
            expected: "String".to_string(),
            found: format!("{:?}", value),
        }),
    }
}

fn value_as_u32(value: Value) -> AtomResult<u32> {
    match value {
        Value::Int(i) => Ok(i as u32),
        Value::Uint(u) => Ok(u),
        _ => Err(AtomError::InvalidType {
            expected: "Integer".to_string(),
            found: format!("{:?}", value),
        }),
    }
}

fn value_as_string_list(value: Value) -> AtomResult<Vec<String>> {
    match value {
        Value::Array(arr) => arr
            .values
            .into_iter()
            .map(value_as_string)
            .collect(),
        Value::Str(s) => Ok(vec![s.to_string()]),
        _ => Err(AtomError::InvalidType {
            expected: "String or Array of Strings".to_string(),
            found: format!("{:?}", value),
        }),
    }
}

fn value_as_task_mode(value: Value) -> AtomResult<TaskMode> {
    let s = value_as_string(value)?;
    match s.as_str() {
        "gsd" => Ok(TaskMode::Gsd),
        "check" => Ok(TaskMode::Check),
        _ => Err(AtomError::ValidationError(format!(
            "invalid task mode '{}', expected 'gsd' or 'check'",
            s
        ))),
    }
}

fn value_as_phase_mode(value: Value) -> AtomResult<PhaseMode> {
    let s = value_as_string(value)?;
    match s.as_str() {
        "serial" => Ok(PhaseMode::Serial),
        "parallel" => Ok(PhaseMode::Parallel),
        _ => Err(AtomError::ValidationError(format!(
            "invalid phase mode '{}', expected 'serial' or 'parallel'",
            s
        ))),
    }
}

/// Validate a handoff path like `phase.run.handoff.field`.
fn validate_handoff_path(path: &str) -> AtomResult<()> {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.len() < 3 {
        return Err(AtomError::ValidationError(format!(
            "input_from path '{}' must have at least 3 segments (phase.run.field)",
            path
        )));
    }
    if parts[2] != "handoff" && parts[2] != "output" {
        return Err(AtomError::ValidationError(format!(
            "input_from path '{}' must use 'handoff' or 'output' as third segment",
            path
        )));
    }
    Ok(())
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_plan_builder() {
        let plan = TaskPlan::new("api_v2")
            .add_phase(
                Phase::new("discovery")
                    .add_run(RunRef::new("discover", "goal-discovery")),
            );
        assert_eq!(plan.id, "api_v2");
        assert_eq!(plan.phases.len(), 1);
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn rejects_duplicate_phase_names() {
        let plan = TaskPlan::new("x")
            .add_phase(Phase::new("a"))
            .add_phase(Phase::new("a"));
        assert!(plan.validate().is_err());
    }

    #[test]
    fn rejects_duplicate_run_names() {
        let plan = TaskPlan::new("x").add_phase(
            Phase::new("a")
                .add_run(RunRef::new("r1", "f"))
                .add_run(RunRef::new("r1", "f")),
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn rejects_unknown_dependency() {
        let plan = TaskPlan::new("x").add_phase(
            Phase::new("a").depends_on(vec!["missing".to_string()]),
        );
        assert!(plan.validate().is_err());
    }

    #[test]
    fn rejects_cycle() {
        let plan = TaskPlan::new("x")
            .add_phase(Phase::new("a").depends_on(vec!["b".to_string()]))
            .add_phase(Phase::new("b").depends_on(vec!["a".to_string()]));
        assert!(plan.validate().is_err());
    }

    #[test]
    fn rejects_invalid_handoff_path() {
        let plan = TaskPlan::new("x").add_phase(
            Phase::new("a").add_run(
                RunRef::new("r1", "f").with_input_from(vec!["bad".to_string()]),
            ),
        );
        assert!(plan.validate().is_err());
    }
}
