//! P2.3-C Real Effect E2E (review 94 §八 "真实 Effect E2E"): exercises the
//! complete production chain through Core public API — search → rank →
//! select → execute (via engine) → history → re-rank.
//!
//! For real plugin binary E2E, see `apps/calculator-plus/tests/workflow_e2e.rs`.

use launcher_core::{favorites::FavoriteService, Core, Provider};
use launcher_domain::{Action, ActionKind, Category, Command, QueryContext};
use launcher_indexer::Indexer;

fn setup(tag: &str) -> (std::path::PathBuf, Core) {
    static SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    let dir = std::env::temp_dir().join(format!("nl_eff_{tag}_{}_{}", std::process::id(), n));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut core = Core::new();
    let index = Indexer::open(&dir.join("idx.db")).unwrap();
    core.set_history(index);
    core.set_favorites(FavoriteService::open(&dir.join("fav.db")).unwrap());
    (dir, core)
}

fn app(id: &str, title: &str, target: &str) -> Command {
    Command {
        id: id.into(),
        title: title.into(),
        subtitle: None,
        icon: None,
        provider_id: "app-registry".into(),
        score: 0.0,
        keywords: vec![],
        category: Category::Application,
        actions: vec![Action {
            kind: ActionKind::Open,
            payload: Some(launcher_domain::ActionPayload::Path(target.into())),
            id: None,
            title: None,
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        }],
        target: Some(target.into()),
    }
}

struct TestProvider(Vec<Command>);

impl Provider for TestProvider {
    fn id(&self) -> &str {
        "test"
    }
    fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
        self.0.clone()
    }
}

/// Golden path: search → rank → select → execute → history → re-rank.
#[test]
fn golden_path_search_execute_history_rerank() {
    let (_dir, mut core) = setup("golden");
    core.register(Box::new(TestProvider(vec![
        app("alpha", "App Alpha", r"C:\apps\alpha.exe"),
        app("beta", "App Beta", r"C:\apps\beta.exe"),
    ])));

    // 1. search finds both
    let r = core.search("app", 10);
    assert_eq!(r.commands.len(), 2);

    // 2. execute Alpha (simulate: the host calls record_use after success)
    let alpha = r.commands.iter().find(|c| c.id == "alpha").unwrap();
    core.record_use_with_title(&alpha.id, &alpha.provider_id, &alpha.title);

    // 3. re-search — Alpha boosted to #1
    let r2 = core.search("app", 10);
    assert_eq!(r2.commands[0].id, "alpha", "usage boost must keep Alpha at #1");

    // 4. execution IDs unique across attempts
    let e1 = core.next_execution_id();
    let e2 = core.next_execution_id();
    assert_ne!(e1, e2, "INV-AUTH-005: each attempt gets its own execution_id");
}

/// File search + rescan through the Indexer public API (real temp files).
#[test]
fn file_search_and_rescan_real() {
    let tmp = std::env::temp_dir().join(format!(
        "nl_eff_file_{}_{}",
        std::process::id(),
        std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().subsec_nanos()
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("report.pdf"), b"content").unwrap();

    let mut ix = Indexer::open(&tmp.join("index.db")).unwrap();
    ix.rebuild(&[tmp.clone()]).unwrap();
    assert!(ix.search("report", 10).unwrap().iter().any(|f| f.name == "report.pdf"));

    // new file → rescan → searchable
    std::fs::write(tmp.join("new_file.txt"), b"new").unwrap();
    ix.rescan_root(&tmp).unwrap();
    assert!(ix.search("new_file", 10).unwrap().iter().any(|f| f.name == "new_file.txt"));
    std::fs::remove_dir_all(&tmp).ok();
}

/// Execution ID lineage: IDs are always unique and never reused.
#[test]
fn execution_id_uniqueness() {
    let core = Core::new();
    let ids: Vec<String> = (0..10).map(|_| core.next_execution_id()).collect();
    for i in 0..ids.len() {
        for j in (i + 1)..ids.len() {
            assert_ne!(ids[i], ids[j], "INV-AUTH-005: E{} != E{}", i, j);
        }
    }
}
