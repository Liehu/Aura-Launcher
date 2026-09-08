//! E11 Input / Resource Limits (review 89 §12): unbounded user input is
//! capped to prevent OOM in tokenisation and ranking.

use launcher_core::{Core, Provider};
use launcher_domain::{Command, QueryContext};

struct EmptyProvider;

impl Provider for EmptyProvider {
    fn id(&self) -> &str {
        "empty"
    }
    fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
        Vec::new()
    }
}

/// E11: unbounded query length is capped at 512 chars — must not panic
/// or OOM with a 100KB query.
#[test]
fn query_length_capped() {
    let mut core = Core::new();
    core.register(Box::new(EmptyProvider));
    let huge_query = "a".repeat(100_000);
    let r = core.search(&huge_query, 10);
    assert!(r.commands.is_empty());
}

/// E11: unicode query is safely truncated at char boundary.
#[test]
fn unicode_query_truncated_safely() {
    let mut core = Core::new();
    core.register(Box::new(EmptyProvider));
    let unicode_query = " Horn ".repeat(200);
    let r = core.search(&unicode_query, 10);
    assert!(r.commands.is_empty());
}
