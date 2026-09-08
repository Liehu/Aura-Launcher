//! P2.3-B Product E2E (review: 84 §"End-to-End Product Closure").
//!
//! The GOLDEN PATH, exercised through the Core public API exactly the way
//! the host drives it — no GUI, no mocks for the data plane:
//!
//! ```text
//! hotkey(popup) → query → rank → select → execute → history
//!     → re-query (boost) → favorite → empty query (recents)
//!     → file index → copy path action present
//! ```

use launcher_core::{favorites::FavoriteService, providers::file::FileProvider, Core, Provider};
use launcher_indexer::Indexer;
use launcher_domain::{
    Action, ActionKind, Category, Command, QueryContext,
};

fn app_cmd(_id: &str, title: &str, target: &str) -> Command {
    Command {
        id: format!("appreg:{target}"),
        title: title.into(),
        subtitle: Some(target.into()),
        icon: None,
        provider_id: "app-registry".into(),
        score: 0.0,
        keywords: vec![title.to_lowercase()],
        category: Category::Application,
        actions: vec![
            Action {
                kind: ActionKind::Open,
                payload: Some(launcher_domain::ActionPayload::Path(target.into())),
                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            },
            Action {
                kind: ActionKind::Copy,
                payload: Some(launcher_domain::ActionPayload::Path(target.into())),
                id: Some("copypath".into()),
                title: Some("Copy path".into()),
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            },
        ],
        target: Some(target.into()),
    }
}

struct StaticProvider {
    id: String,
    cmds: Vec<Command>,
}

impl Provider for StaticProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn query(&mut self, _q: &QueryContext) -> Vec<Command> {
        self.cmds.clone()
    }
}

/// PRODUCT-E2E-001 (review 84 §"End-to-End Product Closure"): the golden
/// path — search → rank → select → execute(simulated effect) → history →
/// re-search boost → favorite → empty-query recents, across app/file
/// providers with a real temp file index.
#[test]
fn golden_path_search_execute_history_favorite() {
    let tmp = std::env::temp_dir().join(format!("nl_p23b_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("report.pdf"), b"pdf").unwrap();
    std::fs::write(tmp.join("notes.txt"), b"txt").unwrap();

    let mut core = Core::new();
    // build the index FIRST (production: startup rescan before first query)
    let mut index = Indexer::open(&tmp.join("index.db")).unwrap();
    index.rebuild(&[tmp.clone()]).unwrap();
    core.set_history(index);
    core.register(Box::new(StaticProvider {
        id: "app-registry".into(),
        cmds: vec![
            app_cmd("chrome", "Google Chrome", r"C:\Apps\chrome.exe"),
            app_cmd("vscode", "VS Code", r"C:\Apps\code.exe"),
        ],
    }));
    // files go through the REAL FileProvider (production path: index query
    // + file_command builder with Open/Reveal/Copy-path actions)
    core.register(Box::new(FileProvider::new(
        Indexer::open(&tmp.join("index.db")).unwrap(),
    )));
    core.set_favorites(FavoriteService::open(&tmp.join("favorites.db")).unwrap());

    // ---- Act 1: file search finds the indexed file with actions ----
    let r = core.search("report", 10);
    assert!(
        r.commands.iter().any(|c| c.title == "report.pdf"),
        "file search must surface indexed files"
    );
    let report = r.commands.iter().find(|c| c.title == "report.pdf").unwrap();
    assert!(
        report.actions.iter().any(|a| a.id.as_deref() == Some("copypath")),
        "file results must carry the Copy path action"
    );

    // ---- Act 2: app search + execute simulation (the host calls
    // record_use after a successful effect — same as execute_action_by_id)
    let r = core.search("chrome", 10);
    let chrome = r.commands.iter().find(|c| c.title == "Google Chrome").unwrap();
    let chrome_id = chrome.id.clone();
    let chrome_provider = chrome.provider_id.clone();
    let chrome_title = chrome.title.clone();
    core.record_use_with_title(&chrome_id, &chrome_provider, &chrome_title);

    // ---- Act 3: re-search — history boost keeps Chrome at #1 ----
    let r = core.search("chrome", 10);
    assert_eq!(r.commands[0].title, "Google Chrome", "usage boost holds #1");

    // ---- Act 4: favorite the app ----
    let cmd = core
        .search("chrome", 10)
        .commands
        .into_iter()
        .find(|c| c.title == "Google Chrome")
        .unwrap();
    let new_state = core.toggle_favorite(&cmd);
    assert_eq!(new_state, Some(true), "favorite toggled on");

    // ---- Act 5: empty query recents include the used/favorited app ----
    let recents = core.recent_commands(10);
    assert!(
        recents.iter().any(|c| c.title == "Google Chrome"),
        "empty query must surface the used app"
    );
    // and recents carry executable actions (INV-SEARCH-003)
    assert!(recents.iter().all(|c| !c.actions.is_empty()));

    // ---- Act 6: non-matching query still returns nothing (boost never
    // resurrects candidates, INV-SEARCH-002) ----
    let r = core.search("zzz-no-match", 10);
    assert!(r.commands.is_empty());

    std::fs::remove_dir_all(&tmp).ok();
}

/// PRODUCT-E2E-002: the file index + real temp files — index, search,
/// rescan after new file, all through the Indexer public API.
#[test]
fn file_index_search_and_rescan() {
    let tmp = std::env::temp_dir().join(format!("nl_p23b_idx_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    std::fs::write(tmp.join("alpha.txt"), "a").unwrap();

    let mut ix = Indexer::open(&tmp.join("index.db")).unwrap();
    ix.rebuild(&[tmp.clone()]).unwrap();
    assert!(ix
        .search("alpha", 10)
        .unwrap()
        .iter()
        .any(|f| f.name == "alpha.txt"));

    // rescan picks up newly created files (P2.1-B recovery path)
    std::fs::write(tmp.join("beta.txt"), "b").unwrap();
    ix.rescan_root(&tmp).unwrap();
    assert!(ix.search("beta", 10).unwrap().iter().any(|f| f.name == "beta.txt"));
    std::fs::remove_dir_all(&tmp).ok();
}

/// PRODUCT-E2E-003 (MUST-1 chain): an installed workflow is discovered by
/// search, its definition validates, and the trigger payload round-trips.
#[test]
fn workflow_catalog_discovery_and_validation() {
    let tmp = std::env::temp_dir().join(format!("nl_p23b_wf_{}", std::process::id()));
    let wf_dir = tmp.join("workflows");
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&wf_dir).unwrap();
    let def = serde_json::json!({
        "id": "wf.e2e", "version": 1, "name": "E2E Workflow",
        "steps": [{ "step_id": "s1", "action": { "Inline": {
            "id": "s1", "title": "Copy", "type": "system.copy_to_clipboard",
            "input": {"text": "hello"} } }, "input": {"text": "hello"} } ]
    });
    std::fs::write(wf_dir.join("e2e.json"), def.to_string()).unwrap();

    let mut provider = launcher_core::providers::workflows::WorkflowCatalogProvider::load_dir(&wf_dir);
    let cmds = provider.query(&launcher_domain::QueryContext::parse("e2e"));
    assert_eq!(cmds.len(), 1, "installed workflow must be discoverable");
    assert_eq!(cmds[0].title, "E2E Workflow");

    // trigger payload round-trip: the action's target must re-validate
    let target = cmds[0].target.as_ref().unwrap();
    let reloaded = launcher_core::providers::workflows::load_definition(
        std::path::Path::new(target),
    ).unwrap();
    assert_eq!(reloaded.name, "E2E Workflow");
    std::fs::remove_dir_all(&tmp).ok();
}
