//! Plan Validator (P27-B03, spec `P2.7 开发设计规范` §14/§39-B03): checks a
//! plan against the Tool Catalog projection (B01) BEFORE any step is
//! submitted to the frozen Resolver → Engine chain. Fail-closed: every
//! step's `action_ref` must exist in the catalog; input objects must be
//! JSON objects; unknown refs never execute (catalog visibility ≠
//! capability grant, §13 — and conversely, an absent ref is never a
//! permission question, it is a plan error).

use crate::agent_contract::PlanStep;
use crate::plan::PlanDocument;
use crate::tool_catalog::{ToolCatalog, ToolKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanValidationError {
    /// `action_ref` not present in the catalog (fail closed).
    UnknownRef { step_id: String, action_ref: String },
    /// The ref names an action but the plan addressed it as a workflow, or
    /// vice versa.
    KindMismatch { step_id: String, expected: &'static str, found: &'static str },
    /// Step input must be a JSON object (or null = defaults).
    InvalidInput { step_id: String },
}

impl std::fmt::Display for PlanValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnknownRef { step_id, action_ref } => {
                write!(f, "step {step_id}: unknown action_ref `{action_ref}`")
            }
            Self::KindMismatch { step_id, expected, found } => {
                write!(f, "step {step_id}: expects {expected}, catalog has {found}")
            }
            Self::InvalidInput { step_id } => {
                write!(f, "step {step_id}: input must be a JSON object or null")
            }
        }
    }
}

/// Validate every step of a plan document against the catalog. The
/// document's own structural rules were validated at load/construction
/// (B02); this pass is the CATALOG pass. Returns the offending step on
/// first failure (deterministic: document order).
pub fn validate_plan(
    doc: &PlanDocument,
    catalog: &ToolCatalog,
) -> Result<(), PlanValidationError> {
    for step in &doc.steps {
        validate_step(step, catalog)?;
    }
    Ok(())
}

/// Validate one step against the catalog.
pub fn validate_step(
    step: &PlanStep,
    catalog: &ToolCatalog,
) -> Result<(), PlanValidationError> {
    let Some(entry) = catalog.get(&step.action_ref) else {
        return Err(PlanValidationError::UnknownRef {
            step_id: step.step_id.clone(),
            action_ref: step.action_ref.clone(),
        });
    };
    let kind_name = match entry.kind {
        ToolKind::Action => "action",
        ToolKind::Workflow => "workflow",
    };
    // A step's ref is self-describing: `workflow|…` refs must resolve to
    // workflow entries, everything else to action entries.
    let addressed_as_workflow = step.action_ref.starts_with("workflow|");
    let expected = if addressed_as_workflow { "workflow" } else { "action" };
    if expected != kind_name {
        return Err(PlanValidationError::KindMismatch {
            step_id: step.step_id.clone(),
            expected,
            found: kind_name,
        });
    }
    if !step.input.is_null() && !step.input.is_object() {
        return Err(PlanValidationError::InvalidInput { step_id: step.step_id.clone() });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent_contract::PlanStep;
    use crate::tool_catalog::{ToolCatalog, WorkflowToolSummary};
    use launcher_workflow::proposal::ActionCatalogItem;
    use serde_json::json;
    use launcher_domain::{Action, ActionKind, Category, Command};

    fn catalog() -> ToolCatalog {
        let c = Command {
            id: "evaluate".into(),
            title: "Evaluate".into(),
            subtitle: None,
            icon: None,
            provider_id: "mcp:calc".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Plugin,
            actions: vec![Action {
                kind: ActionKind::PluginInvoke,
                payload: None,
                id: Some("invoke".into()),
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        };
        ToolCatalog::project(
            &ActionCatalogItem::items_from_command(&c),
            &[WorkflowToolSummary {
                definition_id: "wf.demo".into(),
                name: "Demo".into(),
                description: None,
            }],
        )
    }

    fn step(id: &str, r#ref: &str, input: serde_json::Value) -> PlanStep {
        PlanStep {
            step_id: id.into(),
            action_ref: r#ref.into(),
            input,
            rationale: None,
            requires_approval: false,
        }
    }

    fn doc(steps: Vec<PlanStep>) -> PlanDocument {
        PlanDocument {
            schema_version: crate::plan::PLAN_SCHEMA_VERSION,
            proposal_id: "p1".into(),
            session_id: "s1".into(),
            user_goal: "g".into(),
            steps,
        }
    }

    /// B03: known refs pass; unknown refs fail closed with the step named.
    #[test]
    fn unknown_ref_fails_closed() {
        let cat = catalog();
        let ok = doc(vec![step("a", "mcp:calc|evaluate|invoke", json!({}))]);
        assert_eq!(validate_plan(&ok, &cat), Ok(()));
        let bad = doc(vec![step("a", "command:app:ghost", json!({}))]);
        assert_eq!(
            validate_plan(&bad, &cat),
            Err(PlanValidationError::UnknownRef {
                step_id: "a".into(),
                action_ref: "command:app:ghost".into(),
            })
        );
    }

    /// B03: workflow refs must resolve to workflow entries (and vice versa).
    #[test]
    fn kind_mismatch_detected() {
        let cat = catalog();
        let ok = doc(vec![step("w", "workflow|wf.demo", json!({}))]);
        assert_eq!(validate_plan(&ok, &cat), Ok(()));
        let bad = doc(vec![step("a", "workflow|evaluate", json!({}))]);
        assert!(matches!(
            validate_plan(&bad, &cat),
            Err(PlanValidationError::UnknownRef { .. })
        ));
    }

    /// B03: inputs must be objects or null — arrays/strings fail closed.
    #[test]
    fn non_object_input_rejected() {
        let cat = catalog();
        let bad = doc(vec![step("a", "mcp:calc|evaluate|invoke", json!([1, 2]))]);
        assert_eq!(
            validate_plan(&bad, &cat),
            Err(PlanValidationError::InvalidInput { step_id: "a".into() })
        );
        let ok = doc(vec![step("a", "mcp:calc|evaluate|invoke", serde_json::Value::Null)]);
        assert_eq!(validate_plan(&ok, &cat), Ok(()));
    }
}
