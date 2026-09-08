//! Search quality corpus (docs/SEARCH-QUALITY.md): fixed fixture catalog +
//! fixed queries with expected Top-1, so ranking changes are judged by
//! numbers, not vibes. Run with `cargo test -p launcher-core --test quality`.

use launcher_core::{Core, Provider};
use launcher_domain::{Action, ActionKind, Category, Command, QueryContext};

struct FixtureProvider {
    id: String,
    commands: Vec<Command>,
}

impl Provider for FixtureProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if q.normalized.is_empty() {
            Vec::new()
        } else {
            self.commands.clone()
        }
    }
}

fn app(id: &str, title: &str, keywords: &[&str]) -> Command {
    Command {
        id: format!("app:{id}"),
        title: title.into(),
        subtitle: Some(format!(r"C:\apps\{id}.exe")),
        icon: None,
        provider_id: "apps".into(),
        score: 0.0,
        keywords: keywords.iter().map(|s| s.to_string()).collect(),
        category: Category::Application,
        actions: vec![Action {
            kind: ActionKind::Open,
            payload: None,

            id: None,
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        }],
        target: None,
    }
}

fn file(id: &str, title: &str) -> Command {
    Command {
        id: format!("file:{id}"),
        title: title.into(),
        subtitle: Some(format!(r"C:\docs\{title}")),
        icon: None,
        provider_id: "files".into(),
        score: 0.0,
        keywords: vec![title.to_string()],
        category: Category::File,
        actions: vec![Action {
            kind: ActionKind::Open,
            payload: None,

            id: None,
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        }],
        target: None,
    }
}

/// Fixture catalog standing in for a real machine (docs/SEARCH-QUALITY.md).
fn fixture_core() -> Core {
    let mut core = Core::new();
    core.register(Box::new(FixtureProvider {
        id: "apps".into(),
        commands: vec![
            app("chrome", "Google Chrome", &["chrome", "browser"]),
            app("code", "Visual Studio Code", &["vscode", "code", "editor"]),
            app("calc", "Calculator", &["calc", "calculator"]),
            app("notepad", "Notepad", &["notepad"]),
            app("git", "Git Bash", &["git", "bash"]),
            app("terminal", "Windows Terminal", &["terminal", "term"]),
            app("settings", "Settings", &["sett", "settings", "control"]),
            app("control", "Control Panel", &["control", "panel"]),
            app("downloader", "Download Manager", &["down", "download"]),
        ],
    }));
    core.register(Box::new(FixtureProvider {
        id: "files".into(),
        commands: vec![
            file("1", "chrome-shortcut.url"),
            file("2", "git-notes.md"),
            file("3", "terminal-setup.pdf"),
            file("4", "settings-backup.json"),
        ],
    }));
    core
}

/// (query, expected Top-1 title). Top-1 accuracy must stay 100%.
const CORPUS_TOP1: &[(&str, &str)] = &[
    ("chrome", "Google Chrome"),
    ("ch", "Google Chrome"),
    ("vscode", "Visual Studio Code"),
    ("calc", "Calculator"),
    ("notepad", "Notepad"),
    ("git", "Git Bash"),
    ("terminal", "Windows Terminal"),
    ("sett", "Settings"),
    ("control", "Control Panel"),
    ("down", "Download Manager"),
];

/// (query, must-be-present). Acceptable-set checks for ambiguous queries.
const CORPUS_TOP3: &[(&str, &[&str])] = &[
    ("term", &["Windows Terminal", "terminal-setup.pdf"]),
    ("pdf", &["terminal-setup.pdf"]), // extension in title
    ("code", &["Visual Studio Code"]),
];

#[test]
fn top1_accuracy_is_full() {
    let mut core = fixture_core();
    let mut hits = 0usize;
    for (query, expected) in CORPUS_TOP1 {
        let r = core.search(query, 10);
        let got = r.commands.first().map(|c| c.title.as_str());
        assert_eq!(
            got,
            Some(*expected),
            "query {query:?}: expected top1 {expected:?}"
        );
        hits += 1;
    }
    assert_eq!(hits, CORPUS_TOP1.len());
}

#[test]
fn top3_acceptable_sets() {
    let mut core = fixture_core();
    for (query, acceptable) in CORPUS_TOP3 {
        let r = core.search(query, 10);
        let top3: Vec<&str> = r
            .commands
            .iter()
            .take(3)
            .map(|c| c.title.as_str())
            .collect();
        assert!(
            acceptable.iter().any(|a| top3.contains(a)),
            "query {query:?}: none of {acceptable:?} in top3 {top3:?}"
        );
    }
}

#[test]
fn type_prior_let_applications_beat_files() {
    // equal text match (word-exact "chrome") — the Application prior must
    // decide, not string similarity (docs/SEARCH-QUALITY.md, 07 §10)
    let mut core = Core::new();
    core.register(Box::new(FixtureProvider {
        id: "mixed".into(),
        commands: vec![
            file("svg", "chrome-icon.svg"),
            app("exe", "Chrome", &["chrome"]),
        ],
    }));
    let r = core.search("chrome", 10);
    assert_eq!(
        r.commands[0].title, "Chrome",
        "app must outrank file at equal match"
    );
    assert_eq!(r.commands[1].title, "chrome-icon.svg");
}

#[test]
fn ranking_stays_deterministic_on_corpus() {
    let run = || {
        let mut core = fixture_core();
        CORPUS_TOP1
            .iter()
            .map(|(q, _)| {
                let r = core.search(q, 10);
                r.commands.iter().map(|c| c.id.clone()).collect::<Vec<_>>()
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        run(),
        run(),
        "ranking must not depend on input order/timing"
    );
}

// ---- P2.1 §80/§82: ranking regression corpus (mixed providers) ----------

/// Corpus fixture: same application discoverable through two providers with
/// different naming; a lexically stronger file also matches. Assertions:
/// Top-1 is the application (type prior + word-exact), and NO duplicate
/// identity appears (Duplicate Rate = 0 on this corpus).
#[test]
fn corpus_chrome_mixed_providers_top1_and_no_duplicates() {
    use launcher_core::{Core, Provider};
    use launcher_domain::{Action, ActionKind, Category, Command, QueryContext};

    fn cmd(provider: &str, id: &str, title: &str, category: Category) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: provider.into(),
            score: 0.0,
            keywords: vec![],
            category,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        }
    }

    struct P(&'static str, Vec<Command>);
    impl Provider for P {
        fn id(&self) -> &str {
            self.0
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            self.1.clone()
        }
    }

    let mut core = Core::new();
    core.register(Box::new(P(
        "apps",
        vec![
            cmd("apps", "chrome", "Google Chrome", Category::Application),
            cmd("apps", "chromium", "Chromium", Category::Application),
        ],
    )));
    core.register(Box::new(P(
        "files",
        vec![
            cmd("files", "chrome-icons.svg", "chrome-icons.svg", Category::File),
            cmd("files", "chromium-notes.txt", "chromium-notes.txt", Category::File),
        ],
    )));

    let r = core.search("chrome", 10);
    assert_eq!(r.commands[0].title, "Google Chrome", "Top-1 must be the app");
    // duplicate identity rate on this corpus: 0
    let mut seen = std::collections::HashSet::new();
    for c in &r.commands {
        assert!(
            seen.insert((c.provider_id.clone(), c.id.clone())),
            "duplicate identity in results"
        );
    }
}


/// INV-SEARCH-004 (review 72 §31): one provider failing (reported error)
/// must NOT discard unrelated providers' results.
#[test]
fn provider_error_does_not_discard_other_results() {
    use launcher_core::{Core, Provider};
    use launcher_domain::{Action, ActionKind, Category, Command, QueryContext};
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn cmd(id: &str, title: &str) -> Command {
        Command {
            id: id.into(),
            title: title.into(),
            subtitle: None,
            icon: None,
            provider_id: "apps".into(),
            score: 0.0,
            keywords: vec![],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: None,
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: None,
        }
    }

    struct FailingProvider(AtomicUsize);
    impl Provider for FailingProvider {
        fn id(&self) -> &str {
            "broken"
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            self.0.fetch_add(1, Ordering::SeqCst);
            Vec::new()
        }
        fn take_last_error(&mut self) -> Option<String> {
            Some("mcp server unreachable".into())
        }
    }
    struct Healthy;
    impl Provider for Healthy {
        fn id(&self) -> &str {
            "apps"
        }
        fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
            vec![cmd("chrome", "Google Chrome")]
        }
    }

    let mut core = Core::new();
    core.register(Box::new(FailingProvider(AtomicUsize::new(0))));
    core.register(Box::new(Healthy));
    let r = core.search("chrome", 10);
    assert!(
        r.commands.iter().any(|c| c.id == "chrome"),
        "healthy provider results must survive an unrelated provider failure"
    );
    assert!(
        r.errors.iter().any(|e| e.contains("broken")),
        "the failure must be reported, not swallowed"
    );
}
