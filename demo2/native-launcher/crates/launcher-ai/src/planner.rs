//! LLMPlanner (review 55 §11-§18/§23/§25-§29): the semantic planner.
//!
//! Pipeline: intent → bounded catalog view → LlmProvider → strict
//! structured-output parse → catalog exact binding → ActionProposal[].
//! The proposal schema here is STRICTLY NARROWER than the domain schema
//! (§12): four fields, `deny_unknown_fields`, ≤ max_proposals — forged
//! `authorized`/`confirmed`/`effect`/`resolved_action`/capability fields
//! fail at this boundary instead of being dropped later. Hallucinated
//! routes are discarded by exact (provider, command, action) binding —
//! never fuzzy-repaired (§14/§15/§26).

use std::time::{Duration, Instant};

use launcher_domain::Command;
use launcher_workflow::proposal::{ActionCatalogItem, ActionPlanner, ActionProposal};

use crate::llm::{LlmError, LlmProvider, LlmRequest};
use crate::prompt::build_user_prompt;

/// Strict LLM output schema (§12): `deny_unknown_fields` rejects forged
/// authority fields AT THE SCHEMA LAYER — stronger than strip-after-parse.
#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictProposal {
    provider_id: String,
    command_id: String,
    action_id: String,
    #[serde(default)]
    input: serde_json::Value,
}

#[derive(Debug, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct StrictOutput {
    proposals: Vec<StrictProposal>,
}

/// Planner telemetry (§27): diagnostics, never authority, never enters
/// Workflow.
#[derive(Debug, Clone, Default)]
pub struct PlannerDiagnostics {
    pub provider: String,
    pub latency: Duration,
    pub candidate_count: usize,
    pub generated_count: usize,
    pub rejected_count: usize,
    pub fallback_used: bool,
}

pub struct PlannerResult {
    pub proposals: Vec<ActionProposal>,
    pub diagnostics: PlannerDiagnostics,
}

pub struct LlmPlanner {
    provider: std::sync::Arc<dyn LlmProvider>,
    max_proposals: usize,
    max_catalog_items: usize,
    /// §29: on provider failure, fall back to keyword matching (which can
    /// also yield zero proposals) — never "execute first catalog item".
    fallback_keyword: bool,
}

impl LlmPlanner {
    pub fn new(provider: std::sync::Arc<dyn LlmProvider>) -> Self {
        Self {
            provider,
            max_proposals: 8,
            max_catalog_items: 50,
            fallback_keyword: true,
        }
    }

    pub fn max_proposals(mut self, n: usize) -> Self {
        self.max_proposals = n;
        self
    }

    pub fn max_catalog_items(mut self, n: usize) -> Self {
        self.max_catalog_items = n;
        self
    }

    pub fn fallback_keyword(mut self, enabled: bool) -> Self {
        self.fallback_keyword = enabled;
        self
    }

    /// Full pipeline with diagnostics (§23/§27).
    pub fn plan_with_diagnostics(
        &self,
        input: &str,
        catalog: &[Command],
    ) -> PlannerResult {
        let started = Instant::now();
        let items: Vec<ActionCatalogItem> = catalog
            .iter()
            .flat_map(ActionCatalogItem::items_from_command)
            .collect();
        let candidate_count = items.len();

        let mut diagnostics = PlannerDiagnostics {
            provider: self.provider.name().into(),
            candidate_count,
            ..Default::default()
        };

        // 1. LLM generation
        let prompt = build_user_prompt(input, &items, self.max_catalog_items);
        let text = match self.provider.generate(&LlmRequest {
            system_prompt: crate::prompt::PLANNER_SYSTEM_PROMPT.into(),
            user_prompt: prompt,
        }) {
            Ok(r) => r.text,
            Err(e) => {
                tracing::warn!(provider = %diagnostics.provider, error = %e, "llm planner failed");
                return self.fallback_or_empty(input, catalog, started, diagnostics, &e);
            }
        };

        // 2. strict structured-output parse (§11/§12): invalid JSON, wrong
        // shape, forged extra fields → schema failure here
        let strict: Vec<StrictProposal> = match parse_strict(&text) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!(error = %e, "llm structured output rejected");
                return self.fallback_or_empty(input, catalog, started, diagnostics, &e);
            }
        };

        // 3. cap + catalog exact binding (§14/§15/§25): hallucinated or
        // fuzzy-mismatched routes are discarded, never auto-created
        let mut proposals = Vec::new();
        let mut rejected = 0usize;
        for p in strict.into_iter().take(self.max_proposals) {
            let candidate = ActionProposal {
                provider_id: p.provider_id.clone(),
                command_id: p.command_id.clone(),
                action_id: p.action_id.clone(),
                input: p.input,
            };
            if !route_exists(catalog, &candidate) {
                rejected += 1;
                continue;
            }
            // MCP identity double-binding (§17): input server_id/tool_name
            // must agree with the routed provider/command when present
            let sid = candidate.input.get("server_id").and_then(|v| v.as_str());
            let tname = candidate.input.get("tool_name").and_then(|v| v.as_str());
            if let Some(sid) = sid {
                if candidate.provider_id != format!("mcp:{sid}") {
                    rejected += 1;
                    continue;
                }
            }
            if let Some(tn) = tname {
                if tn != candidate.command_id {
                    rejected += 1;
                    continue;
                }
            }
            proposals.push(candidate);
        }

        diagnostics.latency = started.elapsed();
        diagnostics.generated_count = proposals.len();
        diagnostics.rejected_count = rejected;
        PlannerResult { proposals, diagnostics }
    }

    fn fallback_or_empty(
        &self,
        input: &str,
        catalog: &[Command],
        started: Instant,
        mut diagnostics: PlannerDiagnostics,
        _error: &dyn std::fmt::Display,
    ) -> PlannerResult {
        if self.fallback_keyword {
            let proposals = fallback_keyword_plan(input, catalog);
            diagnostics.fallback_used = true;
            diagnostics.latency = started.elapsed();
            diagnostics.generated_count = proposals.len();
            PlannerResult { proposals, diagnostics }
        } else {
            diagnostics.latency = started.elapsed();
            PlannerResult { proposals: Vec::new(), diagnostics }
        }
    }
}

fn parse_strict(text: &str) -> Result<Vec<StrictProposal>, LlmError> {
    // tolerate markdown fences around the JSON object
    let trimmed = text.trim();
    let json_str = if trimmed.starts_with('{') {
        trimmed
    } else {
        trimmed
            .split("```")
            .find(|seg| seg.trim_start().starts_with('{'))
            .map(|seg| seg.trim_start().trim_start_matches("json").trim_start())
            .ok_or_else(|| LlmError::InvalidOutput("no json object found".into()))?
    };
    let out: StrictOutput = serde_json::from_str(json_str)
        .map_err(|e| LlmError::InvalidOutput(format!("schema: {e}")))?;
    Ok(out.proposals)
}

/// Exact (provider_id, command_id, action_id) existence check against the
/// catalog — the hallucination wall (§14/§25). No fuzzy matching.
fn route_exists(catalog: &[Command], p: &ActionProposal) -> bool {
    catalog.iter().any(|c| {
        c.provider_id == p.provider_id
            && c.id == p.command_id
            && c.actions.iter().any(|a| a.id.as_deref() == Some(p.action_id.as_str()))
    })
}

impl ActionPlanner for LlmPlanner {
    fn plan(&mut self, input: &str, catalog: &[Command]) -> Vec<ActionProposal> {
        self.plan_with_diagnostics(input, catalog).proposals
    }
}

/// keyword fallback helper (reuses the frozen KeywordPlanner).
fn fallback_keyword_plan(input: &str, catalog: &[Command]) -> Vec<ActionProposal> {
    use launcher_workflow::proposal::{ActionPlanner as _, KeywordPlanner};
    KeywordPlanner.plan(input, catalog)
}

