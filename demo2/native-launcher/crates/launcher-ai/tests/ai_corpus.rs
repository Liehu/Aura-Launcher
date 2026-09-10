//! P27-H03: the AI quality corpus (H03, spec §39-H). The corpus is the
//! frozen acceptance set for the deterministic intent parser (A03): every
//! entry asserts the intent AND the entity projection for one input.
//!
//! Contract notes encoded here:
//! - verb-led prefixes route intents; everything else falls back to
//!   Search with the whole (lowercased) input as query — unmatched input
//!   can never be misread as a privileged operation;
//! - injection-flavored user text stays DATA (a Search query); the
//!   structural defenses (fenced prompt sections, proposal validation)
//!   are covered by the G05 matrix.

use launcher_ai::agent_contract::AgentIntent;
use launcher_ai::intent::parse_intent;
use serde::Deserialize;

#[derive(Deserialize)]
struct Case {
    id: String,
    input: String,
    intent: String,
    #[serde(default)]
    query: Option<String>,
    #[serde(default)]
    target: Option<String>,
    #[serde(default)]
    workflow_id: Option<String>,
}

fn intent_of(name: &str) -> AgentIntent {
    match name {
        "search" => AgentIntent::Search,
        "open" => AgentIntent::Open,
        "execute" => AgentIntent::Execute,
        "workflow" => AgentIntent::Workflow,
        "explain" => AgentIntent::Explain,
        other => panic!("unknown intent in corpus: {other}"),
    }
}

#[test]
fn h03_quality_corpus_passes() {
    let raw = include_str!("ai_quality_corpus.json");
    let cases: Vec<Case> = serde_json::from_str(raw).expect("corpus parses");
    assert!(cases.len() >= 20, "corpus grew unexpectedly small");
    for c in &cases {
        let parsed = parse_intent(&c.input);
        assert_eq!(parsed.intent, intent_of(&c.intent), "case {} intent", c.id);
        assert_eq!(parsed.entities.query, c.query, "case {} query", c.id);
        assert_eq!(parsed.entities.target, c.target, "case {} target", c.id);
        assert_eq!(
            parsed.entities.workflow_id, c.workflow_id,
            "case {} workflow_id",
            c.id
        );
    }
}

/// H03: the corpus is deterministic — parsing twice changes nothing
/// (guards against accidental order- or case-dependent regexes).
#[test]
fn h03_corpus_parse_is_stable() {
    let raw = include_str!("ai_quality_corpus.json");
    let cases: Vec<Case> = serde_json::from_str(raw).unwrap();
    for c in &cases {
        let a = parse_intent(&c.input);
        let b = parse_intent(&c.input);
        assert_eq!(a, b, "case {} unstable", c.id);
    }
}
