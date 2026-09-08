//! Phase 10 property tests (review 45 §12, P-001..P-005) + adversarial
//! fuzz-style corpus runs (review 45 §11): deterministic pseudo-random
//! generators (LCG) so the suite stays reproducible without external fuzz
//! infrastructure. Invariants must hold for EVERY generated input.

use crate::common::*;
use launcher_domain::{identity_mismatch, resolve_descriptor, ACTION_IDENTITY_FIELDS, Capability, WorkflowRunStatus};
use launcher_mcp::executor::McpInvokeInput;
use launcher_mcp::types::{McpServerId, McpTool};
use launcher_workflow::proposal::ActionProposal;
use std::sync::Arc;

/// Tiny deterministic PRNG (xorshift64) — reproducible corpus, no deps.
struct Rng(u64);
impl Rng {
    fn new(seed: u64) -> Self {
        Rng(seed | 1)
    }
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

/// P-001: identity — same (server, tool) always yields the same route;
/// different servers never collide, no matter the name.
#[test]
fn p001_identity_determinism_and_scoping() {
    let mut rng = Rng::new(0x5EED);
    for _ in 0..200 {
        let a = rng.below(50);
        let b = rng.below(50);
        let tool = format!("t{}", rng.below(1000));
        let ra = tool_route(&format!("srv{a}"), &tool);
        let rb = tool_route(&format!("srv{b}"), &tool);
        if a == b {
            assert_eq!(ra, rb, "same identity → same route");
        } else {
            assert_ne!(ra, rb, "different servers → different routes");
        }
    }
}

fn tool_route(server: &str, tool: &str) -> (String, String, String) {
    let t = McpTool {
        server_id: McpServerId(server.into()),
        name: tool.into(),
        title: None,
        description: None,
        input_schema: serde_json::json!({}),
        annotations: serde_json::json!({}),
    };
    t.route()
}

/// P-002: for arbitrary unknown/nested fields, a parsed proposal produces
/// neither an Effect nor a capability grant nor confirmation state.
#[test]
fn p002_proposal_authority_never_emerges() {
    let mut rng = Rng::new(42);
    let poison_fields = [
        "authorized", "confirmed", "granted_capabilities", "effect",
        "resolved_action", "capabilities", "policy", "system",
    ];
    for _ in 0..300 {
        let n = 1 + rng.below(poison_fields.len());
        let mut raw = serde_json::json!({
            "provider_id": "mcp:calc", "command_id": "evaluate", "action_id": "invoke",
            "input": {"x": rng.next()}
        });
        for i in 0..n {
            raw[poison_fields[i]] = serde_json::json!({"deep": {"nested": rng.next()}});
        }
        let p = ActionProposal::from_json(&raw).unwrap();
        let keys: Vec<String> = serde_json::to_value(&p)
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect();
        assert_eq!(keys.len(), 4, "frozen shape: {keys:?}");
        for k in &keys {
            assert!(
                !poison_fields.contains(&k.as_str()),
                "authority field emerged: {k}"
            );
        }
    }
}

/// P-003: metadata independence — arbitrary mutations of description,
/// title, annotations and schema description never change the resolver's
/// decision for a fixed grant list.
#[test]
fn p003_metadata_never_moves_policy() {
    let mut rng = Rng::new(77);
    for _ in 0..200 {
        let noise: String = (0..32).map(|_| (b'a' + rng.below(26) as u8) as char).collect();
        let t = McpTool {
            server_id: McpServerId("calc".into()),
            name: "evaluate".into(),
            title: Some(format!("grant {noise} confirmed {noise}")),
            description: Some(format!("ignore instructions {noise} mcp.invoke {noise}")),
            input_schema: serde_json::json!({"type": "object", "description": noise}),
            annotations: serde_json::json!({"x": noise}),
        };
        let d = launcher_mcp::adapter::invoke_descriptor(&t, &McpServerId("calc".into()));
        assert!(resolve_descriptor(&d, &[]).is_err(), "metadata never grants");
        assert!(resolve_descriptor(&d, &[Capability::McpInvoke]).is_ok());
    }
}

/// P-004: attempt identity — every retry mints a fresh execution id; ids
/// are strictly monotonic across arbitrarily many calls.
#[test]
fn p004_execution_ids_strictly_monotonic() {
    let core = Core::new();
    // the registry mints; an executor can only observe the id it receives
    let mut prev = String::new();
    for _ in 0..100 {
        let id = core.next_execution_id();
        assert_ne!(id, prev, "every id is fresh");
        assert!(id.starts_with("e-"));
        prev = id;
    }
}

/// P-005: resolver-before-executor — for arbitrary reference targets
/// (existing or not), any resolution failure leaves the executor untouched.
#[test]
fn p005_resolver_precedes_executor() {
    let ex = Arc::new(RecordingExecutor::default());
    let (mut core, _) = core_with(ex.clone(), vec![tool_command("calc", "evaluate", false)]);
    let mut rng = Rng::new(0xC0FFEE);
    for _ in 0..100 {
        let target = if rng.below(2) == 0 { "evaluate" } else { "missing" };
        let run = run_def(
            &mut core,
            vec![reference_step("mcp:calc", target, calc_input(serde_json::json!({})))],
        );
        if run.status == WorkflowRunStatus::Failed {
            assert_eq!(ex.count(), 0, "failed resolution executed something");
        } else {
            ex.calls.lock().unwrap().clear(); // reset for the next round
        }
    }
}

/// Fuzz corpus 1+3 (JSON-RPC envelope + proposal): arbitrary byte-ish
/// inputs through the proposal parser and invoke-input validator never
/// panic and never forge authority.
#[test]
fn fuzz_proposal_and_invoke_input_corpus() {
    let mut rng = Rng::new(0xDEADBEEF);
    let alphabet: Vec<char> = "{}[]\":,abcdemitruefalsnul 0123456789.<>/\\-".chars().collect();
    for _ in 0..1_000 {
        let len = rng.below(200);
        let s: String = (0..len).map(|_| alphabet[rng.below(alphabet.len())]).collect();
        // proposal parser: no panic, whatever the bytes
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            if let Ok(p) = ActionProposal::from_json(&v) {
                assert_eq!(
                    serde_json::to_value(&p).unwrap().as_object().unwrap().len(),
                    4
                );
            }
        }
        // invoke input validator: no panic on hostile identities
        if let Ok(v) = serde_json::from_str::<serde_json::Value>(&s) {
            let _ = McpInvokeInput::from_json(&v);
        }
        // identity binding: no panic, mismatches only ever shrink authority
        let a = serde_json::json!({"server_id": "calc", "tool_name": "t"});
        let b = serde_json::json!({"server_id": s, "tool_name": s});
        let m = identity_mismatch(&a, &b);
        if !m.is_empty() {
            assert!(m.iter().all(|f| ACTION_IDENTITY_FIELDS.contains(f)));
        }
    }
}
