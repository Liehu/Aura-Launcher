//! Workflow orchestration types (WORKFLOW-CONTRACT-v0.1, ADR-0015).
//!
//! Pure data + policy resolution: no IO, no execution. The persisted objects
//! (Definition / Run / StepRun / ActionInvocation) carry only logical action
//! references or untrusted inline declarations — never ResolvedAction or
//! executable Effects (INV-049/052/053).

use crate::ActionDescriptor;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A logical reference to a published Command/Action (INV-055): resolved
/// against the current popup session, then a fresh provider discovery query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionReference {
    /// Host-assigned provider identity (e.g. `apps`, `plugin:com.example.x`).
    pub provider_id: String,
    /// Plugin-local stable command id; global identity = (provider_id, id).
    pub command_id: String,
    /// Action id within the command.
    pub action_id: String,
}

/// A workflow step's action: reference (external capability) or inline
/// Host-owned declaration (system.* only — WF-A2; plugin.* must use
/// Reference). Inline is a persisted declaration, never trusted executable
/// state (INV-056).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum WorkflowAction {
    Reference(ActionReference),
    Inline(ActionDescriptor),
}

/// AWF-A4: `step_id` MUST be unique within a definition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub step_id: String,
    pub action: WorkflowAction,
    /// THE authoritative execution input (WF-A1): overrides any input carried
    /// by an Inline descriptor (which is declaration/default only).
    /// P1-C: templates (`${var.x}` / `${input.x}`) materialize here.
    #[serde(default)]
    pub input: Value,
    /// Step-level policy override; unset fields fall back to the definition
    /// policy, then to the frozen defaults.
    #[serde(default)]
    pub failure_policy: StepFailurePolicy,
    // ---- v0.2 (P1-C, review 58): optional control-flow/data-flow fields.
    // Absent on every v0.1 workflow — linear semantics unchanged.
    /// Condition-only / gated steps: when present and evaluated FALSE, the
    /// step is skipped (ConditionFalse) and `on_condition_false` applies.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub condition: Option<Condition>,
    /// Output binding: copy a path from the action result into a variable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputBinding>,
    /// Transition taken after SUCCESS (default: Next).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_success: Option<String>,
    /// Transition taken after a FAILURE that the FailurePolicy permits to
    /// continue (e.g. Skip). NEVER overrides Stop (review 58 SS32/SS33).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_failure: Option<String>,
    /// Transition taken when `condition` evaluates FALSE (default: Next).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub on_condition_false: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub id: String,
    #[serde(default = "default_version")]
    pub version: u32,
    pub name: String,
    pub steps: Vec<WorkflowStep>,
    #[serde(default)]
    pub failure_policy: WorkflowFailurePolicy,
    // ---- v0.2 (P1-C, review 58) ----
    /// Entry step id; defaults to the first step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry_step: Option<String>,
    /// Declared workflow variables (schema, not runtime values).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<VariableDeclaration>,
    /// Declared workflow inputs.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub inputs: Vec<WorkflowInputDeclaration>,
}

fn default_version() -> u32 {
    1
}

/// What the runner does after a classified failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureAction {
    Stop,
    Retry,
    Skip,
    ReResolve,
}

/// Bounded retry: max TOTAL attempts per step (default 2 = 1 initial + 1 retry).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RetryPolicy {
    #[serde(default = "default_max_attempts")]
    pub max_attempts: u32,
}

fn default_max_attempts() -> u32 {
    2
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: default_max_attempts(),
        }
    }
}

/// Step-level overrides; `None` inherits definition policy, then defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StepFailurePolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_denied: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_context: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<RetryPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_error: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_violation: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid_input: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_unavailable: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_not_found: Option<FailureAction>,
}

/// Definition-level policy; `None` inherits the frozen defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct WorkflowFailurePolicy {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_denied: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stale_context: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout: Option<RetryPolicy>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub business_error: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub protocol_violation: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invalid_input: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub plugin_unavailable: Option<FailureAction>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_not_found: Option<FailureAction>,
}

/// Frozen default failure matrix (WORKFLOW-CONTRACT §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkflowFailureClass {
    CapabilityDenied,
    InvalidInput,
    StaleContext,
    Timeout,
    BusinessError,
    ProtocolViolation,
    PluginUnavailable,
    CommandNotFound,
    /// Runner-internal: resolution succeeded but the engine demands user
    /// confirmation (MVP3.2 policy) — handled as Run pause, not a policy row.
    ConfirmationRequired,
}

impl WorkflowFailurePolicy {
    /// Default matrix (contract §5): Denied/Invalid/Protocol/NotFound=Stop,
    /// Stale=ReResolve, Timeout/PluginUnavailable=Retry(2), Business=Stop.
    pub fn action_for(
        &self,
        step: &StepFailurePolicy,
        class: WorkflowFailureClass,
    ) -> FailureAction {
        use WorkflowFailureClass as C;
        let (step_override, def_override, default) = match class {
            C::CapabilityDenied => (
                step.capability_denied,
                self.capability_denied,
                FailureAction::Stop,
            ),
            C::InvalidInput => (step.invalid_input, self.invalid_input, FailureAction::Stop),
            C::StaleContext => (
                step.stale_context,
                self.stale_context,
                FailureAction::ReResolve,
            ),
            C::Timeout => (
                step.timeout.as_ref().map(|_| FailureAction::Retry),
                self.timeout.as_ref().map(|_| FailureAction::Retry),
                FailureAction::Retry,
            ),
            C::BusinessError => (
                step.business_error,
                self.business_error,
                FailureAction::Stop,
            ),
            C::ProtocolViolation => (
                step.protocol_violation,
                self.protocol_violation,
                FailureAction::Stop,
            ),
            C::PluginUnavailable => (
                step.plugin_unavailable,
                self.plugin_unavailable,
                FailureAction::Retry,
            ),
            C::CommandNotFound => (
                step.command_not_found,
                self.command_not_found,
                FailureAction::Stop,
            ),
            C::ConfirmationRequired => (None, None, FailureAction::Stop),
        };
        step_override.or(def_override).unwrap_or(default)
    }

    pub fn max_attempts(&self, step: &StepFailurePolicy) -> u32 {
        step.timeout
            .as_ref()
            .map(|r| r.max_attempts)
            .or(self.timeout.as_ref().map(|r| r.max_attempts))
            .unwrap_or_else(default_max_attempts)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepRunStatus {
    Pending,
    Resolving,
    Resolved,
    WaitingForConfirmation,
    Executing,
    Complete,
    Failed,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkflowRunStatus {
    Created,
    Running,
    /// Waiting for an external confirmation event from the originating UI
    /// session (WF-010). NOT a persisted authorization (INV-048).
    Paused,
    Succeeded,
    Failed,
    Cancelled,
}

/// One run of a step: attempt counter, last classified error, last execution
/// id, and the context generation the last successful resolution used.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StepRun {
    pub step_id: String,
    pub status: StepRunStatus,
    #[serde(default)]
    pub attempt: u32,
    #[serde(default)]
    pub last_error: Option<String>,
    #[serde(default)]
    pub last_execution_id: Option<String>,
    #[serde(default)]
    pub resolved_context_generation: Option<u64>,
    /// v0.2: why a step was skipped (BranchNotSelected / ConditionFalse).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skip_reason: Option<String>,
    /// v0.2: captured action-result payload for output binding (SS20).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub workflow_run_id: String,
    pub definition_id: String,
    pub definition_version: u32,
    pub status: WorkflowRunStatus,
    #[serde(default)]
    pub paused_reason: Option<String>,
    pub current_step: Option<String>,
    pub steps: Vec<StepRun>,
    /// v0.2 run-scoped variable snapshot (observability; memory-only, no
    /// disk persistence — review 58 SS42).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub variables: Option<Value>,
}

impl WorkflowDefinition {
    /// INV-059: step_id MUST be unique within a definition.
    pub fn validate(&self) -> Result<(), String> {
        if self.id.trim().is_empty() {
            return Err("workflow id is required".into());
        }
        if self.steps.is_empty() {
            return Err("workflow must have at least one step".into());
        }
        let mut seen = std::collections::BTreeSet::new();
        for s in &self.steps {
            if s.step_id.trim().is_empty() {
                return Err("step_id is required".into());
            }
            if !seen.insert(s.step_id.clone()) {
                return Err(format!("duplicate step_id: {}", s.step_id));
            }
        }
        Ok(())
    }

    pub fn fresh_run(&self, workflow_run_id: String) -> WorkflowRun {
        WorkflowRun {
            workflow_run_id,
            definition_id: self.id.clone(),
            definition_version: self.version,
            status: WorkflowRunStatus::Created,
            paused_reason: None,
            variables: None,
            current_step: None,
            steps: self
                .steps
                .iter()
                .map(|s| StepRun {
            output: None,
                skip_reason: None,
                    step_id: s.step_id.clone(),
                    status: StepRunStatus::Pending,
                    attempt: 0,
                    last_error: None,
                    last_execution_id: None,
                    resolved_context_generation: None,
                })
                .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn step(id: &str) -> WorkflowStep {
        serde_json::from_value(serde_json::json!({
            "step_id": id,
            "action": { "Inline": {
                "id": "copy", "title": "Copy", "type": "system.copy_to_clipboard",
                "input": {"text": "default"} } },
            "input": {"text": "step-input"}
        }))
        .unwrap()
    }

    #[test]
    fn definition_validation() {
        let d = WorkflowDefinition {
            id: "wf.test".into(),
            version: 1,
            name: "T".into(),
            steps: vec![step("s1"), step("s1")],
            failure_policy: Default::default(),
            entry_step: None,
            variables: Vec::new(),
            inputs: Vec::new(),
        };
        assert!(d.validate().is_err()); // duplicate step_id (INV-059)

        let d = WorkflowDefinition { steps: vec![], ..d };
        assert!(d.validate().is_err()); // empty steps
    }

    #[test]
    fn default_failure_matrix() {
        let pol = WorkflowFailurePolicy::default();
        let none = StepFailurePolicy::default();
        assert_eq!(
            pol.action_for(&none, WorkflowFailureClass::CapabilityDenied),
            FailureAction::Stop
        );
        assert_eq!(
            pol.action_for(&none, WorkflowFailureClass::StaleContext),
            FailureAction::ReResolve
        );
        assert_eq!(
            pol.action_for(&none, WorkflowFailureClass::Timeout),
            FailureAction::Retry
        );
        assert_eq!(
            pol.action_for(&none, WorkflowFailureClass::InvalidInput),
            FailureAction::Stop
        );
        assert_eq!(pol.max_attempts(&none), 2);
        // step override wins, then definition override
        let step = StepFailurePolicy {
            invalid_input: Some(FailureAction::Skip),
            ..Default::default()
        };
        assert_eq!(
            pol.action_for(&step, WorkflowFailureClass::InvalidInput),
            FailureAction::Skip
        );
    }

    /// WF-A1: step input is authoritative; inline descriptor input is the
    /// declaration/default and is overridden.
    #[test]
    fn step_input_is_authoritative() {
        let s: WorkflowStep = serde_json::from_value(serde_json::json!({
            "step_id": "s1",
            "action": { "Inline": {
                "id": "copy", "type": "system.copy_to_clipboard",
                "input": {"text": "DECLARED"} } },
            "input": {"text": "STEP"}
        }))
        .unwrap();
        let d: ActionDescriptor = match &s.action {
            WorkflowAction::Inline(d) => d.clone(),
            _ => unreachable!(),
        };
        // the runner merges: step.input wins over the declared default
        let effective = serde_json::json!({"text": "STEP"});
        assert_eq!(d.input, serde_json::json!({"text": "DECLARED"}));
        assert_ne!(d.input, effective);
    }
}

// ---- Workflow v0.2: variables / conditions / branching (P1-C, review 58) ----

/// A step condition: typed expression + error policy. v0.2 error policy is
/// FailWorkflow only — condition evaluation errors are never swallowed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Condition {
    pub expression: String,
}

impl Condition {
    /// The raw expression is reparsed on use — deterministic pure parser,
    /// and the AST never needs to serialize.
    pub fn expression(&self) -> Result<crate::expr::Expr, crate::expr::ExprError> {
        crate::expr::parse_expr(&self.expression)
    }

    pub fn parse(expression: &str) -> Result<Self, crate::expr::ExprError> {
        // validate eagerly so malformed definitions fail at load time
        crate::expr::parse_expr(expression)?;
        Ok(Condition {
            expression: expression.to_string(),
        })
    }
}

/// Output binding: `target` is a `var.<name>` path; `source` is a dotted
/// path into the action result (empty = whole result). A missing source
/// path is `OutputPathNotFound`, never null (review 58 SS22).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutputBinding {
    pub target: String,
    pub source: String,
}

/// Declared workflow variable (schema, not runtime value).
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct VariableDeclaration {
    pub name: String,
    #[serde(default = "default_var_type")]
    pub value_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}

fn default_var_type() -> String {
    "any".into()
}

/// Declared workflow input.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowInputDeclaration {
    pub name: String,
    #[serde(default = "default_var_type")]
    pub value_type: String,
    #[serde(default)]
    pub required: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Value>,
}



/// v0.2 evaluator safety limits (review 58 SS58/SS59): evaluator guards,
/// NOT runtime CPU/RAM quotas.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowLimits {
    #[serde(default = "default_max_steps")]
    pub max_steps_per_run: usize,
    #[serde(default = "default_max_variables")]
    pub max_variables: usize,
    #[serde(default = "default_max_variable_bytes")]
    pub max_variable_bytes: usize,
    #[serde(default = "default_max_expression_depth")]
    pub max_expression_depth: usize,
}

fn default_max_steps() -> usize { 1000 }
fn default_max_variables() -> usize { 256 }
fn default_max_variable_bytes() -> usize { 1024 * 1024 }
fn default_max_expression_depth() -> usize { 32 }

pub const MAX_STEPS_PER_RUN: usize = 1000;
pub const MAX_VARIABLES: usize = 256;
pub const MAX_VARIABLE_BYTES: usize = 1024 * 1024;

impl Default for WorkflowLimits {
    fn default() -> Self {
        Self {
            max_steps_per_run: 1000,
            max_variables: 256,
            max_variable_bytes: 1024 * 1024,
            max_expression_depth: 32,
        }
    }
}

/// Why a step was skipped (review 58 SS25).
pub const SKIP_BRANCH_NOT_SELECTED: &str = "BranchNotSelected";
pub const SKIP_CONDITION_FALSE: &str = "ConditionFalse";
