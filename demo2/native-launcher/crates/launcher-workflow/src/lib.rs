//! Workflow Runner (MVP4.1, WORKFLOW-CONTRACT-v0.1 / ADR-0015).
//!
//! Orchestration only: the runner drives Definition → Run → ActionInvocation
//! → ReferenceResolver → ActionResolver → ActionEngine, applies the failure
//! policy, and never executes effects itself. The host plugs in two ports:
//!
//! - [`CommandSource`]: where to find referenced commands (popup session,
//!   then fresh empty-text discovery query).
//! - [`ActionExecutor`]: how a resolved action becomes an Effect (engine +
//!   PluginBroker routing), returning classified failures.

pub mod proposal;
pub mod graph;
pub mod v2;

use launcher_domain::{
    Action, ActionReference, Command, DescriptorError, FailureAction, StepRunStatus,
    WorkflowAction, WorkflowDefinition, WorkflowFailureClass, WorkflowFailurePolicy, WorkflowRun,
    WorkflowRunStatus, WorkflowStep,
};
use serde_json::Value;

/// Why a step failed, classified per contract §5 (INV-045 mapping is the
/// host adapter's job: PluginError → class before it reaches the runner).
#[derive(Debug, Clone, PartialEq)]
pub struct WfFailure {
    pub class: WorkflowFailureClass,
    pub reason: String,
    pub execution_id: Option<String>,
}

impl WfFailure {
    pub fn new(class: WorkflowFailureClass, reason: impl Into<String>) -> Self {
        Self {
            class,
            reason: reason.into(),
            execution_id: None,
        }
    }

    pub fn with_execution_id(mut self, id: Option<String>) -> Self {
        self.execution_id = id;
        self
    }
}

/// Port 1 (WF-006 / INV-055): where referenced commands live.
pub trait CommandSource: Send {
    /// Option ① — the current popup session results.
    fn in_session(&self, _provider_id: &str, _command_id: &str) -> Option<Command> {
        None
    }

    /// Option ② — fresh empty-text discovery query for one provider
    /// (WF-A3). Err = provider-level failure, already classified.
    fn fresh_query(
        &mut self,
        provider_id: &str,
    ) -> Result<Vec<Command>, (WorkflowFailureClass, String)>;
}

/// Port 2: execute a resolved action through the engine (+ broker routing).
/// `confirmed` = the originating UI session's explicit confirmation for
/// confirmation-required actions (WF-010).
pub trait ActionExecutor: Send {
    fn execute(
        &mut self,
        action: Action,
        provider_id: Option<String>,
        context_generation: u64,
        confirmed: bool,
    ) -> Result<ExecutionOutcome, WfFailure>;
}

#[derive(Debug, Clone)]
pub struct ExecutionOutcome {
    pub execution_id: Option<String>,
    pub plugin_result: Option<Value>,
}

/// ReferenceResolver (review 26 §十一): "在哪里找到 Action" — popup session
/// first, then fresh discovery; never caches descriptors (INV-049).
pub struct ReferenceResolver<'a, S: CommandSource> {
    source: &'a mut S,
}

/// Resolution output: the freshly resolved, engine-ready action.
pub struct ResolvedReference {
    pub command: Command,
    pub action: Action,
}

impl<'a, S: CommandSource> ReferenceResolver<'a, S> {
    pub fn new(source: &'a mut S) -> Self {
        Self { source }
    }

    /// Resolve a reference to a fresh Command+Action. Failure classes:
    /// CommandNotFound (both sources missed) or provider-level classes
    /// surfaced by the source (PluginUnavailable / ProtocolViolation).
    pub fn resolve(&mut self, r: &ActionReference) -> Result<ResolvedReference, WfFailure> {
        // ① popup session
        if let Some(c) = self.source.in_session(&r.provider_id, &r.command_id) {
            let action = c
                .actions
                .iter()
                .find(|a| a.id.as_deref() == Some(r.action_id.as_str()))
                .cloned();
            if let Some(a) = action {
                return Ok(ResolvedReference {
                    command: c,
                    action: a,
                });
            }
        }
        // ② fresh discovery query (empty text = command discovery, WF-A3)
        let commands = self
            .source
            .fresh_query(&r.provider_id)
            .map_err(|(class, reason)| WfFailure::new(class, reason))?;
        let c = commands
            .into_iter()
            .find(|c| c.provider_id == r.provider_id && c.id == r.command_id)
            .ok_or_else(|| {
                WfFailure::new(
                    WorkflowFailureClass::CommandNotFound,
                    format!("{} :: {}", r.provider_id, r.command_id),
                )
            })?;
        let a = c
            .actions
            .iter()
            .find(|a| a.id.as_deref() == Some(r.action_id.as_str()))
            .ok_or_else(|| {
                WfFailure::new(
                    WorkflowFailureClass::CommandNotFound,
                    format!("action {} not on {}", r.action_id, r.command_id),
                )
            })?
            .clone();
        Ok(ResolvedReference {
            command: c,
            action: a,
        })
    }
}

/// Effective per-step policy: step override → definition → frozen default.
fn policy_for(
    def: &WorkflowDefinition,
    step: &WorkflowStep,
    class: WorkflowFailureClass,
) -> FailureAction {
    def.failure_policy.action_for(&step.failure_policy, class)
}

fn max_attempts(def: &WorkflowDefinition, step: &WorkflowStep) -> u32 {
    def.failure_policy.max_attempts(&step.failure_policy)
}

pub struct ResolvedStepAction {
    pub command: Option<Command>,
    pub action: Action,
    /// Host-assigned provider id for broker routing (PluginInvoke only).
    pub provider_id: Option<String>,
}

/// The host port pair every backend implements once (AI/MCP adapters
/// included): where commands live + how effects execute.
pub trait WorkflowHost: CommandSource + ActionExecutor {}

/// The runner: drives Definition → Run → steps → classification → policy.
pub struct WorkflowRunner<H: WorkflowHost> {
    host: H,
}

impl<H: WorkflowHost> WorkflowRunner<H> {
    pub fn new(host: H) -> Self {
        Self { host }
    }

    /// Execute a definition to completion (or pause). Fresh query semantics
    /// and re-resolution counts are bounded by the frozen policy.
    pub fn run(
        &mut self,
        def: &WorkflowDefinition,
        workflow_run_id: String,
        context_generation: u64,
    ) -> Result<WorkflowRun, String> {
        def.validate()?;
        v2::validate_graph(def).map_err(|errs| {
            errs.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("; ")
        })?;
        let mut run = def.fresh_run(workflow_run_id);
        run.status = WorkflowRunStatus::Running;
        self.advance(def, &mut run, context_generation, false)?;
        Ok(run)
    }

    /// Resume a paused run (WF-010): only Paused(ConfirmationRequired) runs
    /// may resume; the pending action is re-resolved and executed as
    /// confirmed. Resume NEVER executes a previously resolved action.
    pub fn resume(
        &mut self,
        def: &WorkflowDefinition,
        mut run: WorkflowRun,
        context_generation: u64,
    ) -> Result<WorkflowRun, String> {
        if run.status != WorkflowRunStatus::Paused {
            return Err("only paused runs can resume".into());
        }
        run.status = WorkflowRunStatus::Running;
        run.paused_reason = None;
        self.advance(def, &mut run, context_generation, true)?;
        Ok(run)
    }

    /// v0.2 (P1-C, review 58 §65): transition-driven execution walk.
    /// Order frozen: condition → input materialization → reference
    /// resolution → engine → output capture → binding → transition →
    /// variable commit. Linear v0.1 workflows (no transitions) walk
    /// identically to before.
    fn advance(
        &mut self,
        def: &WorkflowDefinition,
        run: &mut WorkflowRun,
        context_generation: u64,
        mut confirmed: bool,
    ) -> Result<(), String> {
        let index: std::collections::HashMap<&str, usize> = def
            .steps
            .iter()
            .enumerate()
            .map(|(i, s)| (s.step_id.as_str(), i))
            .collect();

        let entry = def
            .entry_step
            .clone()
            .or_else(|| def.steps.first().map(|s| s.step_id.clone()))
            .unwrap_or_default();
        let mut current: Option<usize> = index.get(entry.as_str()).copied();
        let mut executed: usize = 0;
        let limits_max = launcher_domain::workflow::MAX_STEPS_PER_RUN;

        while let Some(idx) = current {
            executed += 1;
            if executed > limits_max {
                run.status = WorkflowRunStatus::Failed;
                run.steps[idx].last_error =
                    Some(format!("max steps per run exceeded ({limits_max})"));
                return Ok(());
            }
            let step = &def.steps[idx];
            // resume path: steps already Complete/Skipped are not re-run
            if matches!(
                run.steps[idx].status,
                StepRunStatus::Complete | StepRunStatus::Skipped
            ) {
                current = self.next_linear(def, idx);
                continue;
            }
            run.current_step = Some(step.step_id.clone());

            // ---- P1-C condition gate (pure evaluation, §46) ----
            if let Some(cond) = &step.condition {
                let vars_snapshot = run.variables.clone();
                let store = v2::VariableStore::from_snapshot(vars_snapshot.as_ref());
                let expr = match cond.expression() {
                    Ok(e) => e,
                    Err(e) => {
                        run.status = WorkflowRunStatus::Failed;
                        run.steps[idx].status = StepRunStatus::Failed;
                        run.steps[idx].last_error =
                            Some(format!("condition error: {e}"));
                        return Ok(());
                    }
                };
                let value = launcher_domain::expr::evaluate(
                    &expr,
                    &|path| store.get(path).unwrap_or(None),
                )
                .map_err(|e| format!("condition error: {e}"))?;
                let truthy = matches!(value, launcher_domain::Value::Bool(true));
                if !truthy {
                    {
                        let sr = &mut run.steps[idx];
                        sr.status = StepRunStatus::Skipped;
                        sr.skip_reason =
                            Some(launcher_domain::workflow::SKIP_CONDITION_FALSE.into());
                    }
                    current = match step.on_condition_false.as_deref() {
                        Some("end") => None,
                        Some(target) => index.get(target).copied(),
                        None => self.next_linear(def, idx),
                    };
                    if current.is_none() {
                        // fall out of the walk: run end (nothing left)
                        break;
                    }
                    continue;
                }
                // condition TRUE: fall through to execution
            }

            match self.execute_step(def, step, run, idx, context_generation, &mut confirmed) {
                StepOutcome::Next => {
                    // P1-C output binding (§19-§22): on success, copy the
                    // bound path from the captured action output into the
                    // variable store. A binding failure fails the step and
                    // commits NOTHING (SS49 transaction rule).
                    if run.steps[idx].status == StepRunStatus::Complete {
                        if let Some(binding) = &step.output {
                            let bound = run.steps[idx]
                                .output
                                .as_ref()
                                .and_then(|out| {
                                    if binding.source.is_empty() {
                                        Some(out.clone())
                                    } else {
                                        v2::json_walk(out, &binding.source)
                                            .cloned()
                                    }
                                });
                            let Some(value) = bound else {
                                run.steps[idx].status = StepRunStatus::Failed;
                                run.steps[idx].last_error = Some(format!(
                                    "output path not found: {}",
                                    binding.source
                                ));
                                run.status = WorkflowRunStatus::Failed;
                                return Ok(());
                            };
                            let mut store = v2::VariableStore::from_snapshot(
                                run.variables.as_ref(),
                            );
                            if let Err(e) = store.set(&binding.target, value) {
                                run.steps[idx].status = StepRunStatus::Failed;
                                run.steps[idx].last_error = Some(e);
                                run.status = WorkflowRunStatus::Failed;
                                return Ok(());
                            }
                            run.variables = store.snapshot();
                        }
                    }
                    // P1-C transitions after a Next outcome (SS32/SS33):
                    // - a Skipped step (policy permitted continuation) may
                    //   have an explicit failure transition
                    // - otherwise the success transition applies
                    let skipped = run.steps[idx].status == StepRunStatus::Skipped;
                    let transition = if skipped {
                        step.on_failure.as_deref()
                    } else {
                        step.on_success.as_deref()
                    };
                    current = match transition {
                        Some("end") => None,
                        Some(t) => index.get(t).copied(),
                        None => self.next_linear(def, idx),
                    };
                    if current.is_none() {
                        break;
                    }
                }
                StepOutcome::Paused => {
                    run.status = WorkflowRunStatus::Paused;
                    run.paused_reason = Some("ConfirmationRequired".into());
                    return Ok(());
                }
                StepOutcome::Failed => {
                    run.status = WorkflowRunStatus::Failed;
                    return Ok(());
                }
            }
        }
        // every reachable step is Complete or Skipped; unselected branch
        // steps (still Pending) are marked Skipped(BranchNotSelected)
        for sr in &mut run.steps {
            if sr.status == StepRunStatus::Pending {
                sr.status = StepRunStatus::Skipped;
                sr.skip_reason =
                    Some(launcher_domain::workflow::SKIP_BRANCH_NOT_SELECTED.into());
            }
        }
        let all_done = run
            .steps
            .iter()
            .all(|s| matches!(s.status, StepRunStatus::Complete | StepRunStatus::Skipped));
        run.status = if all_done {
            WorkflowRunStatus::Succeeded
        } else {
            WorkflowRunStatus::Failed
        };
        Ok(())
    }

    /// The linear successor of step `idx` (v0.1 semantics), or None at the
    /// end of the definition.
    fn next_linear(&self, def: &WorkflowDefinition, idx: usize) -> Option<usize> {
        if idx + 1 < def.steps.len() {
            Some(idx + 1)
        } else {
            None
        }
    }

    fn execute_step(
        &mut self,
        def: &WorkflowDefinition,
        step: &WorkflowStep,
        run: &mut WorkflowRun,
        idx: usize,
        context_generation: u64,
        confirmed: &mut bool,
    ) -> StepOutcome {
        let sr = &mut run.steps[idx];
        sr.status = StepRunStatus::Resolving;
        sr.attempt += 1;

        // WF-A1 materialization: templates in step.input resolve against the
        // run-scoped variable store BEFORE resolution; unknown references are
        // a hard error (never null). Non-templated inputs pass through.
        let store = v2::VariableStore::from_snapshot(run.variables.as_ref());
        let materialized =
            v2::materialize_input(&step.input, &store).unwrap_or(step.input.clone());

        // --- ReferenceResolver: where to find the action (WF-006/INV-055)
        // --- then ActionResolver semantics: freshness + capability/policy
        let mut re_resolves: u32 = 0;
        let resolved = loop {
            let outcome = match &step.action {
                WorkflowAction::Reference(r) => {
                    let mut resolver = ReferenceResolver::new(&mut self.host);
                    resolver.resolve(r).and_then(|rr| {
                        let provider_id = (rr.action.kind
                            == launcher_domain::ActionKind::PluginInvoke)
                            .then(|| rr.command.provider_id.clone());
                        let mut action = rr.action;
                        // WF-A1 applies to references too (Phase 8 §17):
                        // step.input is THE authoritative runtime input;
                        // the resolved descriptor's payload is declaration
                        // only. Routing identity stays host-side.
                        let projected_identity = match action.payload.as_ref() {
                            Some(launcher_domain::ActionPayload::Json(v)) => Some(v.clone()),
                            _ => None,
                        };
                        if !materialized.is_null() {
                            action.payload =
                                Some(launcher_domain::ActionPayload::Json(materialized.clone()));
                        }
                        // Phase 10 (review 45 A-003): the runtime input must
                        // not re-point the resolved route at another server
                        // or tool (tool substitution). Generic identity
                        // binding, defined once in launcher-domain.
                        if let (Some(proj), Some(launcher_domain::ActionPayload::Json(rt))) =
                            (projected_identity, action.payload.as_ref())
                        {
                            let mismatch = launcher_domain::identity_mismatch(&proj, rt);
                            if !mismatch.is_empty() {
                                return Err(WfFailure::new(
                                    WorkflowFailureClass::InvalidInput,
                                    format!(
                                        "runtime input alters resolved route identity: {}",
                                        mismatch.join(", ")
                                    ),
                                ));
                            }
                        }
                        Ok(ResolvedStepAction {
                            command: Some(rr.command),
                            action,
                            provider_id,
                        })
                    })
                }
                WorkflowAction::Inline(d) => {
                    // WF-A2: Inline must be Host-owned system.*; routing
                    // plugin.* through own_plugin=None yields UnknownActionType
                    // -> InvalidInput (never a second execution channel).
                    // WF-A1: step.input overrides the declared default input.
                    let mut dd = d.clone();
                    if !materialized.is_null() {
                        dd.input = materialized.clone();
                    }
                    match launcher_domain::resolve_descriptor(&dd, &[]) {
                        Ok(mut a) => {
                            a.id = Some(d.id.clone());
                            a.title = d.title.clone();
                            Ok(ResolvedStepAction {
                                command: None,
                                action: a,
                                provider_id: None,
                            })
                        }
                        Err(e) => Err(classify_descriptor_error(e)),
                    }
                }
            };
            let outcome = match outcome {
                Ok(r) => {
                    // capability denial at resolution time (INV-027): the
                    // action is presentable in the UI but never executable
                    if let Some(reason) = r.action.disabled_reason.clone() {
                        Err(WfFailure::new(
                            WorkflowFailureClass::CapabilityDenied,
                            format!("action disabled: {reason}"),
                        ))
                    } else {
                        Ok(r)
                    }
                }
                Err(f) => Err(f),
            };
            match outcome {
                Ok(r) => break r,
                Err(f) => {
                    sr.last_error = Some(f.reason.clone());
                    match policy_for(def, step, f.class) {
                        FailureAction::ReResolve if re_resolves < 2 => {
                            // fresh context/source, resolve again (bounded)
                            re_resolves += 1;
                            sr.status = StepRunStatus::Resolving;
                            continue;
                        }
                        FailureAction::Retry if sr.attempt < max_attempts(def, step) => {
                            // new attempt = new execution id later (INV-054)
                            sr.status = StepRunStatus::Resolving;
                            sr.attempt += 1;
                            continue;
                        }
                        FailureAction::Skip => {
                            sr.status = StepRunStatus::Skipped;
                            return StepOutcome::Next;
                        }
                        _ => {
                            sr.status = StepRunStatus::Failed;
                            return StepOutcome::Failed;
                        }
                    }
                }
            }
        };

        let mut action = resolved.action;
        if let Some(c) = &resolved.command {
            // context generation bookkeeping (m5 term)
            let sr = &mut run.steps[idx];
            sr.resolved_context_generation = Some(context_generation);
            let _ = c;
        }
        let sr = &mut run.steps[idx];
        sr.status = StepRunStatus::Resolved;

        // Confirmation policy (MVP3.2 / WF-010): pause, never auto-confirm
        if action.confirmation_required && !*confirmed {
            sr.status = StepRunStatus::WaitingForConfirmation;
            sr.last_error = Some("confirmation required".into());
            return StepOutcome::Paused;
        }
        action.confirmation_required = false; // host confirmed (second Enter)

        // --- ActionExecutor: engine (+ broker) with the single gateway
        sr.status = StepRunStatus::Executing;
        let provider_id = resolved.provider_id.clone();
        match self
            .host
            .execute(action, provider_id, context_generation, *confirmed)
        {
            Ok(outcome) => {
                let sr = &mut run.steps[idx];
                sr.status = StepRunStatus::Complete;
                sr.last_execution_id = outcome.execution_id;
                sr.resolved_context_generation = Some(context_generation);
                // P1-C SS20: capture the action-result payload for the
                // step's output binding (data, never authority)
                sr.output = outcome.plugin_result;
                StepOutcome::Next
            }
            Err(f) => {
                let budget_ok = re_resolve_budget(run, idx);
                let attempts = run.steps[idx].attempt;
                let max = max_attempts(def, step);
                run.steps[idx].last_error = Some(f.reason.clone());
                run.steps[idx].last_execution_id = f.execution_id.clone();
                match policy_for(def, step, f.class) {
                    FailureAction::Retry if attempts < max => {
                        // the recursive call's entry increment records the
                        // next attempt (single counting)
                        run.steps[idx].status = StepRunStatus::Resolving;
                        // retry loops back through resolution (INV-050)
                        self.execute_step(def, step, run, idx, context_generation, confirmed)
                    }
                    FailureAction::ReResolve if budget_ok => {
                        run.steps[idx].status = StepRunStatus::Resolving;
                        self.execute_step(def, step, run, idx, context_generation, confirmed)
                    }
                    FailureAction::Skip => {
                        run.steps[idx].status = StepRunStatus::Skipped;
                        StepOutcome::Next
                    }
                    _ => {
                        run.steps[idx].status = StepRunStatus::Failed;
                        StepOutcome::Failed
                    }
                }
            }
        }
    }
}

fn re_resolve_budget(run: &WorkflowRun, idx: usize) -> bool {
    // bounded re-resolution: at most one extra resolve after an executing failure
    run.steps[idx].attempt <= 2
}

enum StepOutcome {
    Next,
    Paused,
    Failed,
}

impl<T: CommandSource + ActionExecutor> WorkflowHost for T {}

fn classify_descriptor_error(e: DescriptorError) -> WfFailure {
    match e {
        DescriptorError::UnknownActionType(t) => WfFailure::new(
            WorkflowFailureClass::InvalidInput,
            format!("unknown action type: {t}"),
        ),
        DescriptorError::InvalidInput(t) => WfFailure::new(WorkflowFailureClass::InvalidInput, t),
        DescriptorError::CapabilityDenied(c) => {
            WfFailure::new(WorkflowFailureClass::CapabilityDenied, format!("{c:?}"))
        }
    }
}

/// Execute validated proposals through the EXACT frozen orchestration path
/// (proposal → step → reference resolution → resolver → engine). This is the
/// only way a planner's output becomes an Effect — there is no AI branch in
/// Core (ADR-0016).
pub fn execute_proposals<H: WorkflowHost>(
    host: H,
    proposals: &[proposal::ActionProposal],
    workflow_id: &str,
    context_generation: u64,
) -> Result<launcher_domain::WorkflowRun, String> {
    let steps: Vec<WorkflowStep> = proposals
        .iter()
        .enumerate()
        .map(|(i, p)| p.to_step(format!("step-{i}")))
        .collect();
    let def = WorkflowDefinition {
        id: workflow_id.to_string(),
        version: 1,
        name: workflow_id.to_string(),
        steps,
        failure_policy: WorkflowFailurePolicy::default(),
        entry_step: None,
        variables: Vec::new(),
        inputs: Vec::new(),
    };
    def.validate()?;
    let mut runner = WorkflowRunner::new(host);
    runner.run(&def, format!("wr-{workflow_id}"), context_generation)
}
