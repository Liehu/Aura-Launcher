//! Bridge between the frozen Workflow Runner (launcher-workflow) and the
//! live Core: implements `CommandSource` (popup session → fresh empty-text
//! discovery, WF-006/INV-055) and `ActionExecutor` (engine gate →
//! PluginBroker routing, INV-044/045), mapping native errors into the
//! frozen 8-class failure taxonomy (contract §5).

use launcher_domain::workflow::WorkflowFailureClass as Class;
use launcher_domain::{Action, Command, QueryContext};
use launcher_workflow::{CommandSource, ExecutionOutcome};

use crate::Core;

/// Outcome of executing one resolved action (after broker routing).
pub enum EffectOutcome {
    Done,
    /// `plugin.*` effect: the engine accepted it; the broker executed the
    /// `execute_action` RPC and returned the plugin payload.
    Plugin(serde_json::Value),
}

/// Provider identity match: raw id (`apps`, ...) or plugin form
/// `plugin:<manifest.id>` — both are host-assigned (INV-029).
fn provider_matches(raw_id: &str, want: &str) -> bool {
    raw_id == want
        || raw_id
            .strip_prefix("plugin:")
            .map(|pid| want == pid || format!("plugin:{pid}") == want)
            .unwrap_or(false)
}

/// Adapter over a mutable Core plus the current popup-session results.
pub struct CoreWorkflowBackend<'a> {
    pub core: &'a mut Core,
    /// Snapshot of the current popup result set (option ① of WF-006).
    pub session_results: &'a [Command],
    /// Bounded discovery limit for fresh queries (WF-A3).
    pub discovery_limit: usize,
}

impl<'a> CommandSource for CoreWorkflowBackend<'a> {
    fn in_session(&self, provider_id: &str, command_id: &str) -> Option<Command> {
        self.session_results
            .iter()
            .find(|c| c.provider_id == provider_id && c.id == command_id)
            .cloned()
    }

    fn fresh_query(&mut self, provider_id: &str) -> Result<Vec<Command>, (Class, String)> {
        // empty text = command discovery (WF-A3), bounded
        let q = QueryContext::parse("");
        let mut found = Vec::new();
        let mut matched = false;
        for p in self.core.providers_mut() {
            // namespace-agnostic host mapping: `plugin:<pid>` and `mcp:<pid>`
            // both resolve through the provider's host-assigned identity —
            // the resolver/runner stays provider-agnostic (Phase 8, §3)
            let matches = provider_matches(p.id(), provider_id)
                || p.plugin_identity()
                    .map(|pid| provider_id.ends_with(&format!(":{pid}")))
                    .unwrap_or(false);
            if !matches {
                continue;
            }
            matched = true;
            for c in p.query(&q) {
                if c.provider_id == provider_id && found.len() < self.discovery_limit {
                    found.push(c);
                }
            }
        }
        if !matched {
            // unknown provider: discovery cannot even be attempted
            return Err((
                Class::PluginUnavailable,
                format!("provider not installed: {provider_id}"),
            ));
        }
        Ok(found)
    }
}

impl<'a> launcher_workflow::ActionExecutor for CoreWorkflowBackend<'a> {
    fn execute(
        &mut self,
        action: Action,
        provider_id: Option<String>,
        context_generation: u64,
        _confirmed: bool,
    ) -> Result<ExecutionOutcome, launcher_workflow::WfFailure> {
        let fail = |class: Class, reason: String| launcher_workflow::WfFailure::new(class, reason);
        // confirmation gate is checked by the engine (single choke point)
        match launcher_action::execute(&action) {
            Err(e) => {
                use launcher_action::ActionError as AE;
                match &e {
                    AE::ConfirmationRequired => Err(fail(
                        Class::ConfirmationRequired,
                        "confirmation required".into(),
                    )),
                    AE::Disabled(r) => Err(fail(
                        Class::CapabilityDenied,
                        format!("action disabled: {r}"),
                    )),
                    _ => Err(fail(Class::InvalidInput, e.to_string())),
                }
            }
            Ok(launcher_action::Effect::PluginInvoked { action_id, input }) => {
                // effect routing (MVP4.3 Phase 7, review 41 §7.3): the
                // host-assigned provider namespace picks the executor —
                // `plugin:*` → PluginBroker, `mcp:*` → McpExecutor. The
                // engine never meets an executor directly (§7.4). One
                // execution_id per attempt, minted here (review 42 §12).
                let provider = provider_id.unwrap_or_default();
                let execution_id = self.core.next_execution_id();
                match self.core.execute_effect(
                    &provider,
                    &action_id.unwrap_or_default(),
                    &input,
                    &execution_id,
                    context_generation,
                ) {
                    Ok(result) => Ok(ExecutionOutcome {
                        execution_id: Some(execution_id),
                        plugin_result: Some(serde_json::json!({
                            "provider": provider,
                            "result": result,
                        })),
                    }),
                    // registry/executor failures arrive pre-classified (contract §5)
                    Err((class, reason)) => Err(fail(class, reason)),
                }
            }
            Ok(_) => Ok(ExecutionOutcome {
                execution_id: None,
                plugin_result: None,
            }),
        }
    }
}
