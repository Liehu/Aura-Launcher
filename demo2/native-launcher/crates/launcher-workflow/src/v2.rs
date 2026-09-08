//! Workflow v0.2 control-flow engine (MVP4.4 P1-C, review 58): variables,
//! typed templates, conditions and static graph validation.
//!
//! Frozen boundaries (§54 INV-WORKFLOW-001..010 / SS72):
//! 1. Variables are DATA, never authority.
//! 2. Conditions are pure computation — no fs/network/process/clock.
//! 3. Branching is control flow, not execution authorization.
//! 4. Variables may fill Action INPUT; they can never alter canonical
//!    Action identity (server_id / tool_name / provider_id / command_id /
//!    action_id stay definition-bound — INV-WORKFLOW-IDENTITY-001).
//! 5. FailurePolicy outranks FailureTransition.
//! 6. The runner reaches execution only via
//!    ReferenceResolver → ActionResolver → ActionEngine.

use std::collections::BTreeMap;

use serde_json::Value;

use crate::WorkflowDefinition;

// ---------------- VariableStore ----------------

/// Run-scoped variable store. Backed by the run's variable snapshot:
/// `{"var": {...}, "input": {...}}` as a serde_json object. System
/// namespaces (`workflow.*`, `step.*`) are read-only observability.
#[derive(Debug, Clone, Default)]
pub struct VariableStore {
    root: serde_json::Value,
}

pub const NS_VAR: &str = "var";
pub const NS_INPUT: &str = "input";

/// Walk a dotted path inside a serde_json object ("a.b.c"); None = missing.
pub fn json_walk<'a>(root: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut cur = root;
    for seg in path.split('.') {
        cur = cur.get(seg)?;
    }
    Some(cur)
}

impl VariableStore {
    pub fn from_snapshot(snapshot: Option<&serde_json::Value>) -> Self {
        let root = match snapshot {
            Some(v) if v.is_object() => v.clone(),
            _ => serde_json::json!({}),
        };
        Self { root }
    }

    pub fn snapshot(&self) -> Option<serde_json::Value> {
        match &self.root {
            serde_json::Value::Object(o) if o.is_empty() => None,
            other => Some(other.clone()),
        }
    }

    /// Resolve a namespaced reference (`var.x`, `var.a.b`, `input.src`).
    /// `Ok(None)` = not set; Err = unknown namespace.
    pub fn get(&self, path: &str) -> Result<Option<launcher_domain::Value>, String> {
        let (ns, rest) = split_ns(path)?;
        match ns {
            NS_VAR | NS_INPUT => {}
            "workflow" | "step" => return Ok(Some(launcher_domain::Value::Null)),
            other => return Err(format!("unknown variable namespace {other}")),
        }
        if rest.is_empty() {
            return Ok(self.root.get(ns).map(launcher_domain::Value::from_json));
        }
        Ok(json_walk(&self.root, &format!("{ns}.{rest}"))
            .map(launcher_domain::Value::from_json))
    }

    /// Set `var.<name>`. Refuses system namespaces and enforces the
    /// variable-count budget.
    pub fn set(&mut self, path: &str, value: serde_json::Value) -> Result<(), String> {
        let (ns, rest) = split_ns(path)?;
        if ns != NS_VAR {
            return Err(format!(
                "namespace {ns} is read-only; variables live under var.*"
            ));
        }
        if rest.is_empty() || rest.contains('.') {
            return Err("use a single-segment var.<name> in v0.2".into());
        }
        if !self.root.is_object() {
            self.root = serde_json::json!({});
        }
        let obj = self.root.as_object_mut().unwrap();
        let ns_obj = obj
            .entry(NS_VAR.to_string())
            .or_insert_with(|| serde_json::json!({}));
        let m = ns_obj.as_object_mut().ok_or_else(|| {
            "variable namespace is not an object".to_string()
        })?;
        if m.len() >= launcher_domain::workflow::MAX_VARIABLES {
            return Err("too many variables".into());
        }
        m.insert(rest.to_string(), value);
        Ok(())
    }

    /// Byte size of the snapshot (§59 max_variable_bytes).
    pub fn snapshot_bytes(&self) -> usize {
        serde_json::to_string(&self.root).map(|s| s.len()).unwrap_or(usize::MAX)
    }
}

fn split_ns(path: &str) -> Result<(&str, &str), String> {
    let (ns, rest) = path
        .split_once('.')
        .ok_or_else(|| format!("bare variable {path:?} forbidden; use var.*/input.*"))?;
    Ok((ns, rest))
}

// ---------------- Template resolver ----------------

/// Materialize `${var.x}` / `${input.x}` templates inside a step input
/// (review 58 §39/§40):
/// - a string that is EXACTLY one template restores the referenced
///   Value's type (number stays number);
/// - templates embedded in a longer string interpolate as text;
/// - unknown references are a hard error (never null);
/// - `server_id` / `tool_name` keys are canonical identity and reject
///   templates outright (INV-WORKFLOW-IDENTITY-001).
pub fn materialize_input(
    input: &serde_json::Value,
    store: &VariableStore,
) -> Result<serde_json::Value, String> {
    match input {
        serde_json::Value::String(s) => materialize_string(s, store),
        serde_json::Value::Array(a) => {
            let mut out = Vec::with_capacity(a.len());
            for item in a {
                out.push(materialize_input(item, store)?);
            }
            Ok(Value::Array(out))
        }
        Value::Object(o) => {
            let mut out = BTreeMap::new();
            for (k, v) in o {
                if (k == "server_id" || k == "tool_name")
                    && v.as_str().map(|s| s.contains("${")).unwrap_or(false)
                {
                    return Err(format!(
                        "identity field {k} must be definition-bound; templates forbidden"
                    ));
                }
                out.insert(k.clone(), materialize_input(v, store)?);
            }
            let mut m = serde_json::Map::new();
            for (k, v) in out {
                m.insert(k, v);
            }
            Ok(serde_json::Value::Object(m))
        }
        other => Ok(other.clone()),
    }
}

fn materialize_string(
    s: &str,
    store: &VariableStore,
) -> Result<serde_json::Value, String> {
    let trimmed = s.trim();
    if let Some(inner) = trimmed.strip_prefix("${").and_then(|r| r.strip_suffix('}')) {
        // whole-field template: restore the referenced value's TYPE
        // (number stays number — review 58 SS39/SS40)
        let v = store
            .get(inner)?
            .ok_or_else(|| format!("unknown variable reference {inner}"))?;
        return Ok(v.to_json());
    }
    if !s.contains("${") {
        return Ok(serde_json::Value::String(s.to_string()));
    }
    // embedded interpolation → String
    let mut out = String::new();
    let mut rest = s;
    while let Some(start) = rest.find("${") {
        out.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('}') else {
            return Err(format!("unterminated template in {s:?}"));
        };
        let path = &after[..end];
        let v = store
            .get(path)?
            .ok_or_else(|| format!("unknown variable reference {path}"))?;
        match v {
            launcher_domain::Value::String(t) => out.push_str(&t),
            launcher_domain::Value::Number(n) => out.push_str(&format!("{n}")),
            launcher_domain::Value::Bool(b) => out.push_str(&format!("{b}")),
            _ => return Err(format!("variable {path} is not string-interpolatable")),
        }
        rest = &after[end + 1..];
    }
    out.push_str(rest);
    Ok(serde_json::Value::String(out))
}

// ---------------- Definition validator (v0.2 graph) ----------------

/// Definition-level errors (review 58 §55). Never confusable with runtime
/// execution errors (§56).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DefinitionError {
    #[error("duplicate step id: {0}")]
    DuplicateStepId(String),
    #[error("unknown step id: {0}")]
    UnknownStep(String),
    #[error("cycle detected at step: {0}")]
    CycleDetected(String),
    #[error("unreachable step: {0}")]
    UnreachableStep(String),
    #[error("entry step not found: {0}")]
    EntryNotFound(String),
    #[error("invalid condition expression: {0}")]
    InvalidExpression(String),
}

/// Static validation of the v0.2 control-flow graph (review 58 §28-§30):
/// unique ids, existing transition targets, entry existence, reachability
/// and cycle detection. v0.1 linear workflows (no transitions) trivially
/// pass.
pub fn validate_graph(def: &WorkflowDefinition) -> Result<(), Vec<DefinitionError>> {
    let mut errors = Vec::new();
    let ids: std::collections::BTreeSet<&str> =
        def.steps.iter().map(|s| s.step_id.as_str()).collect();
    if ids.len() != def.steps.len() {
        errors.push(DefinitionError::DuplicateStepId("(see steps)".into()));
    }
    let entry = def
        .entry_step
        .clone()
        .unwrap_or_else(|| def.steps.first().map(|s| s.step_id.clone()).unwrap_or_default());
    if !ids.contains(entry.as_str()) {
        errors.push(DefinitionError::EntryNotFound(entry.clone()));
    }
    // collect transition edges
    let mut edges: Vec<(String, String, &'static str)> = Vec::new();
    for (i, step) in def.steps.iter().enumerate() {
        for (kind, target) in [
            ("success", step.on_success.as_deref()),
            ("failure", step.on_failure.as_deref()),
            ("condition_false", step.on_condition_false.as_deref()),
        ] {
            if let Some(t) = target {
                if t != "end" && !ids.contains(t) {
                    errors.push(DefinitionError::UnknownStep(format!(
                        "{} -> {t} ({kind})"
                    , step.step_id)));
                } else if t == "end" {
                    // explicit terminal is fine
                }
                edges.push((step.step_id.clone(), t.to_string(), kind));
            }
        }
        // v0.1 linear continuation: a step without explicit success/
        // condition-false transitions flows to its linear successor, so
        // plain linear workflows reach every step.
        let has_explicit_flow = step.on_success.is_some() || step.on_condition_false.is_some();
        if !has_explicit_flow {
            if let Some(next) = def.steps.get(i + 1) {
                edges.push((step.step_id.clone(), next.step_id.clone(), "linear"));
            }
        }
        if let Some(cond) = &step.condition {
            if cond.expression().is_err() {
                errors.push(DefinitionError::InvalidExpression(format!(
                    "{}: {}",
                    step.step_id, cond.expression
                )));
            }
        }
    }
    // cycle detection (DFS from entry; only steps reachable from entry)
    let adj: std::collections::HashMap<&str, Vec<&str>> = {
        let mut m: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();
        for (i, step) in def.steps.iter().enumerate() {
            let mut targets: Vec<&str> = Vec::new();
            if let Some(t) = step.on_success.as_deref() {
                targets.push(t);
            }
            if let Some(t) = step.on_condition_false.as_deref() {
                targets.push(t);
            }
            // v0.1 linear continuation: match the edges list so plain linear
            // workflows reach every step (see the collection pass above).
            if step.on_success.is_none() && step.on_condition_false.is_none() {
                if let Some(next) = def.steps.get(i + 1) {
                    targets.push(next.step_id.as_str());
                }
            }
            m.insert(step.step_id.as_str(), targets);
        }
        m
    };
    let mut state: std::collections::HashMap<&str, u8> = std::collections::HashMap::new();
    let mut has_cycle = false;
    fn dfs<'a>(
        node: &'a str,
        adj: &std::collections::HashMap<&'a str, Vec<&'a str>>,
        state: &mut std::collections::HashMap<&'a str, u8>,
        has_cycle: &mut bool,
    ) {
        if *has_cycle {
            return;
        }
        state.insert(node, 1);
        if let Some(targets) = adj.get(node) {
            for t in targets.clone() {
                match state.get(t).copied() {
                    Some(1) => *has_cycle = true,
                    Some(_) => {}
                    None => dfs(t, adj, state, has_cycle),
                }
            }
        }
        state.insert(node, 2);
    }
    dfs(entry.as_str(), &adj, &mut state, &mut has_cycle);
    if has_cycle {
        errors.push(DefinitionError::CycleDetected(entry.clone()));
    }
    // reachability: steps not reachable from entry are definition-invalid
    let mut reachable: std::collections::BTreeSet<&str> = std::collections::BTreeSet::new();
    fn mark<'a>(
        node: &'a str,
        adj: &std::collections::HashMap<&'a str, Vec<&'a str>>,
        seen: &mut std::collections::BTreeSet<&'a str>,
    ) {
        if !seen.insert(node) {
            return;
        }
        if let Some(targets) = adj.get(node) {
            for t in targets.clone() {
                if t != "end" {
                    mark(t, adj, seen);
                }
            }
        }
    }
    mark(entry.as_str(), &adj, &mut reachable);
    for step in &def.steps {
        if !reachable.contains(step.step_id.as_str()) {
            errors.push(DefinitionError::UnreachableStep(step.step_id.clone()));
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

