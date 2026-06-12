//! TaskPlan parser — thin wrapper around `auto_atom::AtomParser`.

use auto_atom::{AtomError, AtomParser, AtomResult};
use crate::relay::task_plan::TaskPlan;

/// Parse Atom text into a `TaskPlan`.
///
/// # Examples
///
/// ```rust
/// use auto_forge::relay::task_plan_parser::parse_task_plan;
///
/// let input = r#"
/// task_plan(id: "x", version: 1) {
///     phase(name: "p1") {
///         run(name: "r1", flow_id: "post-discovery")
///     }
/// }
/// "#;
/// let plan = parse_task_plan(input).unwrap();
/// assert_eq!(plan.id, "x");
/// ```
pub fn parse_task_plan(input: &str) -> AtomResult<TaskPlan> {
    let atom = AtomParser::parse(input)?;
    let node = atom_to_node(atom)?;
    TaskPlan::try_from(node)
}

fn atom_to_node(atom: auto_atom::Atom) -> AtomResult<auto_val::Node> {
    match atom {
        auto_atom::Atom::Node(node) => Ok(node),
        other => Err(AtomError::InvalidType {
            expected: "Node".to_string(),
            found: format!("{:?}", other.to_value()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn example_input() -> &'static str {
        r#"
        task_plan(id: "api_v2", version: 1) {
            title: "Build v2 API"
            default_mode: "gsd"

            phase(name: "discovery") {
                mode: "serial"
                run(name: "discover", flow_id: "goal-discovery") {
                    input: "Build the v2 API."
                }
            }

            phase(name: "design") {
                mode: "serial"
                depends_on: ["discovery"]
                run(name: "architecture", flow_id: "post-discovery") {
                    input_from: ["discovery.discover.handoff.goals"]
                }
            }

            phase(name: "implementation") {
                mode: "parallel"
                depends_on: ["design"]
                run(name: "auth_module", flow_id: "post-discovery") {
                    input_from: "design.architecture.handoff.specs"
                    context: "Focus on auth."
                }
                run(name: "billing_module", flow_id: "post-discovery") {
                    input_from: "design.architecture.handoff.specs"
                    context: "Focus on billing."
                }
            }
        }
        "#
    }

    #[test]
    fn parse_full_example() {
        let plan = parse_task_plan(example_input()).unwrap();
        assert_eq!(plan.id, "api_v2");
        assert_eq!(plan.version, 1);
        assert_eq!(plan.title.as_deref(), Some("Build v2 API"));
        assert_eq!(plan.phases.len(), 3);

        let discovery = &plan.phases[0];
        assert_eq!(discovery.name, "discovery");
        assert_eq!(discovery.runs.len(), 1);
        assert_eq!(discovery.runs[0].name, "discover");

        let design = &plan.phases[1];
        assert_eq!(design.depends_on, vec!["discovery"]);

        let implementation = &plan.phases[2];
        assert_eq!(implementation.mode, crate::relay::task_plan::PhaseMode::Parallel);
        assert_eq!(implementation.runs.len(), 2);
    }

    #[test]
    fn parse_example_v2_file() {
        let input = include_str!("../../../.autoforge/task_plans/example-v2-api.atom");
        let plan = parse_task_plan(input).unwrap();
        assert_eq!(plan.id, "example_v2_api");
        assert_eq!(plan.phases.len(), 5);
    }

    #[test]
    fn rejects_missing_id() {
        let input = r#"task_plan { }"#;
        assert!(parse_task_plan(input).is_err());
    }

    #[test]
    fn rejects_bad_mode() {
        let input = r#"task_plan(id: "x") { default_mode: "fast" }"#;
        assert!(parse_task_plan(input).is_err());
    }

    #[test]
    fn rejects_cycle() {
        let input = r#"
        task_plan(id: "x") {
            phase(name: "a", depends_on: ["b"]) { run(name: "r", flow_id: "f") }
            phase(name: "b", depends_on: ["a"]) { run(name: "r", flow_id: "f") }
        }
        "#;
        assert!(parse_task_plan(input).is_err());
    }
}
