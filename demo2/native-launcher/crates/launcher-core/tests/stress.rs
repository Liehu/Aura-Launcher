//! Provider stress test (test plan 9.5): 100 virtual providers, repeated
//! query rounds. Guards: latency stays bounded and registry stays stable.

use std::time::{Duration, Instant};

use launcher_core::{Core, Provider};
use launcher_domain::{Action, ActionKind, Category, Command, QueryContext};

struct MockProvider {
    id: String,
    commands: Vec<Command>,
}

impl Provider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if q.normalized.is_empty() {
            return Vec::new();
        }
        self.commands.clone()
    }
}

fn mock_provider(idx: usize) -> MockProvider {
    let id = format!("mock{idx}");
    let commands = (0..10)
        .map(|j| Command {
            id: format!("{id}:{j}"),
            title: format!("provider{idx} item{j}"),
            subtitle: None,
            icon: None,
            provider_id: id.clone(),
            score: 0.0,
            keywords: vec![],
            category: Category::Command,
            actions: vec![Action {
                kind: ActionKind::Execute,
                payload: None,

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        })
        .collect();
    MockProvider { id, commands }
}

#[test]
fn one_hundred_providers_latency_and_stability() {
    let mut core = Core::new();
    for i in 0..100 {
        core.register(Box::new(mock_provider(i)));
    }
    assert_eq!(core.provider_count(), 100);

    // warm up
    let _ = core.search("item5", 50);

    // latency guard: fan-out over 100 providers (1000 commands) must stay fast
    let rounds = 20;
    let mut total = Duration::ZERO;
    for _ in 0..rounds {
        let started = Instant::now();
        let r = core.search("item5", 50);
        total += started.elapsed();
        assert_eq!(r.commands.len(), 50);
    }
    let avg = total / rounds;
    assert!(
        avg < Duration::from_millis(50),
        "average query latency {avg:?} exceeded budget"
    );

    // repeated rounds with distinct queries: registry must not degrade or leak
    for i in 0..200 {
        let r = core.search(&format!("provider{}", i % 100), 50);
        assert!(!r.commands.is_empty());
    }
}
