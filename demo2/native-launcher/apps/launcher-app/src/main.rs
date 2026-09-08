//! launcher-app: MVP binary merging UI + Core (allowed for v0.1, spec 3.1).
//!
//! Vertical slice: hotkey -> popup -> search (apps/files/plugins/context)
//! -> keyboard select -> Enter -> Action Engine -> effect.
//!
//! demo1-ported features: TOML config (hotkey/theme/autostart/index dirs),
//! tray icon resident + quit, registry Uninstall app enumeration, recent
//! files provider, panic hook, rolling file logs.

mod autostart;
mod keyboard_walkthrough;
mod win_platform;
mod snapshot;
mod visual_scenarios;
mod workflow_service;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use launcher_action::Effect;
use launcher_config::AppConfig;
use launcher_context::windows_source::WindowsForegroundSource;
use launcher_context::ContextEngine;
use launcher_context::ContextSource as _;
use launcher_core::{providers::file::FileProvider, providers::plugin::PluginProvider, Core};
use launcher_domain::Command;
use launcher_hotkey::GlobalHotkey;
use launcher_indexer::Indexer;
use launcher_providers::{AppRegistryProvider, RecentFilesProvider};
use launcher_ui::{AppWindow, ResultItem, TrayIcon};
use slint::ComponentHandle as _;
use tracing::{info, warn};

fn data_dir() -> PathBuf {
    let mut p = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    p.push("native-launcher");
    let _ = std::fs::create_dir_all(&p);
    p
}

fn default_index_dirs() -> Vec<PathBuf> {
    std::env::var_os("USERPROFILE")
        .map(PathBuf::from)
        .map(|home| {
            ["Documents", "Desktop", "Downloads"]
                .iter()
                .map(|s| home.join(s))
                .filter(|d| d.exists())
                .collect()
        })
        .unwrap_or_default()
}

fn build_core(
    cfg: &AppConfig,
    db_dir: &std::path::Path,
    degraded_boot: bool,
) -> anyhow::Result<(Core, launcher_core::providers::context::ContextHandle)> {
    let mut core = Core::new();

    // context-aware provider (MVP2.1 Context Suggestions)
    let (context_provider, context_handle) =
        launcher_core::providers::context::ContextProvider::new();
    core.register(Box::new(context_provider));

    // apps: Start Menu .lnk + registry Uninstall keys + packaged (MSIX/UWP)
    // + configured portable roots — one Application Catalog (P2.1-D).
    let mut entries = AppRegistryProvider::collect_entries();
    entries.extend(launcher_providers::packaged::collect_packaged_entries(512));
    if !cfg.portable_roots.is_empty() {
        let roots: Vec<PathBuf> = cfg
            .portable_roots
            .iter()
            .map(PathBuf::from)
            .filter(|p| p.exists())
            .collect();
        entries.extend(launcher_providers::app_registry::portable_entries(
            &roots, 128, 2,
        ));
    }
    // P2.4-A03: discovery output becomes ApplicationObservations; the
    // catalog merges them into canonical identities (pure, deterministic).
    use launcher_providers::app_identity::ApplicationObservation;
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0);
    let observations: Vec<ApplicationObservation> = entries
        .iter()
        .map(|e| ApplicationObservation {
            source_id: e.source.to_string(),
            source_kind: e.source.to_string(),
            identity_hint: e.path.display().to_string(),
            display_name: e.name.clone(),
            launch_target: Some(
                e.resolved_target
                    .as_ref()
                    .unwrap_or(&e.path)
                    .display()
                    .to_string(),
            ),
            observed_at: now_ms,
            ..Default::default()
        })
        .collect();
    let (apps, catalog_generation) = {
        let catalog_db = db_dir.join("catalog.db");
        let store = launcher_providers::catalog::CatalogStore::open(&catalog_db)?;
        let (gen, n) = store.reconcile_observations(&observations)?;
        info!(catalog_generation = gen, entries = n, "application catalog persisted");
        // P2.4-A05: the provider is built FROM the committed catalog — the
        // authoritative read path. Fallback: an unreadable/empty catalog
        // must not break the app provider (discovery result is still in hand).
        let from_catalog = store.list().ok().filter(|r| !r.is_empty());
        match from_catalog {
            Some(records) => (
                AppRegistryProvider::from_catalog_records(&records),
                Some(gen),
            ),
            None => (AppRegistryProvider::from_entries(entries), None),
        }
    };
    info!(apps = apps.len(), catalog_authoritative = catalog_generation.is_some(), "application catalog ready");
    core.register(Box::new(apps));
    // P2.4-A05: cache invalidation adopts the COMMITTED catalog generation.
    match catalog_generation {
        Some(gen) => core.set_application_generation(gen),
        None => core.bump_application_generation(),
    }



    // recent files (demo1 port)
    let mut recent = RecentFilesProvider::new();
    match recent.build_cache() {
        Ok(()) => core.register(Box::new(recent)),
        Err(e) => tracing::warn!(error = %e, "recent files provider unavailable"),
    }

    // FIX-05 (review 68 §16-18): startup must never block on a filesystem
    // scan. Open the existing index (fast), serve queries from it
    // immediately, and run the full rebuild on a BACKGROUND connection
    // (WAL: readers keep seeing the last committed snapshot until commit).
    let indexer = Indexer::open(&db_dir.join("index.db"))?;
    let mut roots = default_index_dirs();
    for extra in &cfg.index_dirs {
        let p = PathBuf::from(extra);
        if p.exists() {
            roots.push(p);
        }
    }
    // NOTE: the rebuild thread is spawned AFTER every DB connection below is
    // open — schema init writes must not race the rebuild's write txn.
    // installed workflows (MUST-1): <data>/workflows/*.json surfaced as
    // searchable commands; a demo definition is seeded on first run
    let workflows_dir = db_dir.join("workflows");
    std::fs::create_dir_all(&workflows_dir)?;
    if std::fs::read_dir(&workflows_dir)?.next().is_none() {
        let demo = crate::workflow_service::demo_definition();
        if let Ok(json) = serde_json::to_string_pretty(&demo) {
            let _ = std::fs::write(workflows_dir.join("demo.json"), json);
        }
    }
    let wf_provider =
        launcher_core::providers::workflows::WorkflowCatalogProvider::load_dir(&workflows_dir);
    info!(workflows = wf_provider.len(), "workflow catalog ready");
    core.register(Box::new(wf_provider));

    // P2.2-A: favorites store (semantic identity keys, SQLite)
    core.set_favorites(launcher_core::favorites::FavoriteService::open(
        &db_dir.join("favorites.db"),
    )?);

    // settings entry point (MUST-2): searchable "Open Settings" command
    let config_file = launcher_config::config_path()?;
    core.register(Box::new(SettingsProvider::new(&config_file)));

    // the indexer connection doubles as the bounded history sink (WAL)
    core.set_history(indexer);
    core.register(Box::new(FileProvider::new(Indexer::open(
        &db_dir.join("index.db"),
    )?)));
    // FIX-05 (cont.): all connections are open now — safe to start the
    // background rebuild (WAL: readers keep the last committed snapshot).
    if !roots.is_empty() && cfg.watch_enabled && !degraded_boot {
        // P2.1-B: IndexCoordinator — initial rescan + live watcher +
        // incremental maintenance (INV-INDEX-006: search stays queryable)
        let coordinator_cfg = launcher_indexer::coordinator::CoordinatorConfig {
            roots: roots.clone(),
            ..Default::default()
        };
        let handle = launcher_indexer::coordinator::spawn(&db_dir.join("index.db"), coordinator_cfg);
        std::thread::Builder::new()
            .name("index-status".into())
            .spawn(move || loop {
                if !handle.is_running() {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_secs(30));
                if let Ok(s) = handle.status.lock() {
                    info!(
                        health = ?s.health,
                        generation = s.generation,
                        indexed = s.indexed_entries,
                        pending = s.pending_events,
                        "index.status"
                    );
                }
            })
            .ok();
        info!("index coordinator started (watch + incremental)");
    } else if degraded_boot {
        warn!("DEGRADED BOOT: index coordinator skipped (crash-loop recovery; delete startup_state.json to reset)");
    }


    // MCP servers from config (MVP4.3): each configured server becomes a
    // discovery provider AND an executor endpoint in the Core registry
    // (review 41 §7.3). Execution still flows only through the engine's
    // effect routing.
    for m in &cfg.mcp_servers {
        match m.transport.as_str() {
            "stdio" | "streamable-http" => {}
            other => {
                tracing::warn!(server = %m.id, transport = other, "unsupported mcp transport, skipped");
                continue;
            }
        }
        // Phase 11: wire profile is config-owned; unknown revisions fail
        // closed (server skipped with a WARN, never silently degraded).
        let profile = match launcher_mcp::compat::McpProtocolProfile::parse(&m.profile) {
            Some(p) => p,
            None => {
                tracing::warn!(server = %m.id, profile = %m.profile, "unknown mcp profile, server skipped");
                continue;
            }
        };
        // P0-B: transport selection is configuration data (INV-TRANSPORT-002)
        if m.transport == "streamable-http" {
            let Some(url) = m.url.as_deref() else {
                tracing::warn!(server = %m.id, "streamable-http server missing url, skipped");
                continue;
            };
            if profile != launcher_mcp::compat::McpProtocolProfile::V2026_07_28 {
                tracing::warn!(server = %m.id, profile = profile.as_str(),
                    "streamable-http requires the 2026-07-28 profile, server skipped");
                continue;
            }
            info!(server = %m.id, url, "mcp http server configured");
            core.register_mcp_http_server(
                &m.id,
                url,
                m.allow_private_network,
                m.allow_plain_http,
            );
            core.register(Box::new(launcher_core::providers::mcp::McpProvider::new_http(
                m.id.clone(),
                url.to_string(),
                m.allow_private_network,
                m.allow_plain_http,
            )));
        } else {
            info!(server = %m.id, program = %m.program, profile = profile.as_str(), "mcp server configured");
            let mut provider = launcher_core::providers::mcp::McpProvider::new(
                m.id.clone(),
                m.program.clone(),
                m.args.clone(),
            );
            provider.set_profile(profile);
            // P1-A: persistence is explicit opt-in (review 56 SS23); the
            // default stays ephemeral (runtime-on-demand)
            let endpoint = launcher_mcp::executor::ServerEndpoint::stdio(
                m.program.clone(),
                m.args.clone(),
            )
            .with_profile(profile);
            let endpoint = match m.runtime.as_str() {
                "persistent" => endpoint.with_runtime(
                    launcher_mcp::executor::EndpointRuntime::Persistent,
                ),
                _ => endpoint,
            };
            core.register_mcp_server_with_runtime(&m.id, endpoint);
            core.register(Box::new(provider));
        }
    }

    // plugin manifests: <data>/plugins/*/plugin.json
    // P2.2-D: persistent registry for plugin lifecycle state
    let registry = std::sync::Arc::new(
        launcher_core::providers::plugin_registry::PluginRegistry::open(
            &db_dir.join("plugins.db"),
        )?,
    );
    let plugins_dir = db_dir.join("plugins");
    if let Ok(entries) = std::fs::read_dir(&plugins_dir) {
        for e in entries.flatten() {
            let mf = e.path().join("plugin.json");
            if mf.exists() {
                match PluginProvider::from_manifest_file(&mf) {
                    Ok(mut p) => {
                        // P2.2-D: attach persistent registry state (enabled/
                        // quarantine/failures survive restarts)
                        p.set_registry(registry.clone());
                        // Python script plugins: interpreter resolution order
                        // is config.python_path > LAUNCHER_PYTHON > "python"
                        // (env-expanded in the plugin host, ADR-0009).
                        if p.manifest().is_python() {
                            // Runtime Discovery (ADR-0010): config > env
                            // override > system PATH; the source is logged so
                            // multi-Python setups are diagnosable.
                            let (interp, source) = match cfg.python_path.clone() {
                                Some(v) => (v, "config"),
                                None => match std::env::var("LAUNCHER_PYTHON") {
                                    Ok(v) if !v.is_empty() => (v, "env:LAUNCHER_PYTHON"),
                                    _ => ("python".into(), "path"),
                                },
                            };
                            info!(runtime = "python", source, interpreter = %interp, "runtime.discovered");
                            p.set_interpreter_with_source(PathBuf::from(interp), source);
                        }
                        info!(plugin = %p.manifest().id, "plugin provider registered");
                        core.register(Box::new(p));
                    }
                    Err(err) => tracing::warn!(manifest = %mf.display(), %err, "plugin rejected"),
                }
            }
        }
    }
    Ok((core, context_handle))
}

/// Start-menu `.lnk` entries via the original demo2 AppProvider scan, merged
/// into the registry provider's catalog so both sources share one provider.

/// `#RRGGBB` -> Slint color; invalid config falls back to default with WARN.
fn parse_theme_color(s: &str) -> slint::Color {
    let hex = s.trim_start_matches('#');
    if hex.len() == 6 {
        if let (Ok(r), Ok(g), Ok(b)) = (
            u8::from_str_radix(&hex[0..2], 16),
            u8::from_str_radix(&hex[2..4], 16),
            u8::from_str_radix(&hex[4..6], 16),
        ) {
            return slint::Color::from_rgb_u8(r, g, b);
        }
    }
    tracing::warn!(color = %s, "invalid theme color in config, using default");
    slint::Color::from_rgb_u8(0x5B, 0x8D, 0xEF)
}

/// Resolution-adaptive UI scale (Spec v0.2 addendum): primary screen's
/// LOGICAL height vs the 1080p baseline, quantized to 0.25, clamped
/// [1.0, 2.0]. Dividing by Slint's DPI scale factor avoids double-scaling on
/// displays where Windows scaling already enlarges logical pixels.
#[cfg(windows)]
fn apply_ui_scale(ui: &AppWindow) {
    use windows::Win32::UI::WindowsAndMessaging::{
        GetSystemMetrics, SM_CYSCREEN,
    };
    let physical_h = unsafe { GetSystemMetrics(SM_CYSCREEN) }.max(1) as f32;
    let slint_scale = ui.window().scale_factor().max(1.0);
    let raw = physical_h / 1080.0 / slint_scale;
    let scale = ((raw * 4.0).round() / 4.0).clamp(1.0, 2.0);
    ui.global::<launcher_ui::Theme>().set_ui_scale(scale);
    tracing::debug!(physical_h, slint_scale, ui_scale = scale, "ui.scale");
}

#[cfg(not(windows))]
fn apply_ui_scale(_ui: &AppWindow) {}

struct AppState {
    /// P2.1-E5: icon pipeline (async extraction, L1/L2 cached)
    icons: launcher_providers::icons::IconService,
    core: Core,
    selected: usize,
    /// Foreground window before the popup took focus (captured at popup
    /// open); restored whenever the popup closes so the user continues where
    /// they were (MVP3.1 focus return).
    prev_foreground: Option<isize>,
    /// Context generation (MVP3.2-C): bumped on every popup open; results
    /// record the generation they were resolved under. A stale generation
    /// MUST NOT silently execute (INV-043/045).
    context_gen: u64,
    results_gen: u64,
    /// Pending confirmation (MVP3.2-B): (command_id, action_id) awaiting the
    /// second Enter. Confirmation is host-owned execution policy (INV-041).
    pending_confirmation: Option<(String, String)>,
    /// Single source of truth for executable results (INV-016): the UI only
    /// renders titles + command ids; id -> Command resolution lives here.
    current_results: Vec<Command>,
}

impl AppState {
    fn command_for_id(&self, id: &str) -> Option<Command> {
        self.current_results.iter().find(|c| c.id == id).cloned()
    }
}

/// P2.1 §57/58: an empty query is a first-class view — show the most
/// recently/frequently used commands (full executable actions) instead of
/// "no results". Never "search everything".
/// P2.1-E5 (INV-ICON-002/003): icon extraction is fully asynchronous —
/// results publish immediately with an empty placeholder, icons fill in.
/// Snapshot mode (LAUNCHER_SNAPSHOT_DIR) skips icons entirely so the VR
/// baselines stay deterministic. One serialized pass over the result set;
/// the IconService dedupes/cache per key.
#[allow(clippy::too_many_arguments)]
/// P2.1-E5 (INV-ICON-002/003): icon extraction is fully asynchronous —
/// results publish immediately with an empty placeholder, icons fill in.
/// Snapshot mode (LAUNCHER_SNAPSHOT_DIR) skips icons entirely so the VR
/// baselines stay deterministic. The extraction thread only carries raw
/// RGBA bytes (slint::Image is !Send); images are constructed inside the
/// event loop from `current_results` + extracted pixels.
fn spawn_icon_refresh(state: Arc<Mutex<AppState>>, ui_weak: slint::Weak<AppWindow>, snapshot_mode: bool) {
    if snapshot_mode {
        return;
    }
    std::thread::spawn(move || {
        let (_, icons) = {
            let st = state.lock().expect("state lock");
            let mut icons: Vec<(usize, Vec<u8>, u32, u32)> = Vec::new();
            for (i, c) in st.current_results.iter().enumerate() {
                let Some(target) = c.target.as_ref() else { continue };
                if !target.to_lowercase().ends_with(".exe") {
                    continue; // v0.1: executable sources only
                }
                let key = launcher_domain::IconKey {
                    application: format!(
                        "win32:{}",
                        launcher_domain::normalize_path_identity(target)
                    ),
                    variant: launcher_domain::IconVariant::Normal,
                    source_revision: 0,
                    extractor_version: launcher_providers::icons::EXTRACTOR_VERSION,
                };
                let req = launcher_providers::icons::IconRequest {
                    key,
                    source_path: PathBuf::from(target),
                };
                if let Ok(bmp) = st.icons.get(&req) {
                    icons.push((i, bmp.rgba.to_vec(), bmp.width, bmp.height));
                }
            }
            (st.current_results.clone(), icons)
        };
        if icons.is_empty() {
            return;
        }
        let _ = slint::invoke_from_event_loop(move || {
            let Ok(st) = state.lock() else { return };
            let mut items =
                launcher_ui::to_result_items(st.current_results.iter().cloned(), 50);
            drop(st);
            for (i, rgba, w, h) in &icons {
                if *i < items.len() {
                    let buf = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(
                        rgba, *w, *h,
                    );
                    items[*i].icon_data = slint::Image::from_rgba8(buf);
                }
            }
            if let Some(ui) = ui_weak.upgrade() {
                ui.set_results(slint::ModelRc::new(slint::VecModel::from(items)));
            }
        });
    });
}

fn spawn_search_or_recent(
    state: Arc<Mutex<AppState>>,
    session: Arc<launcher_core::SearchSession>,
    ui_weak: slint::Weak<AppWindow>,
    query: String,
    limit: usize,
) {
    if query.trim().is_empty() {
        spawn_recent(state, session, ui_weak, limit);
    } else {
        spawn_search(state, session, ui_weak, query, limit);
    }
}

fn spawn_recent(
    state: Arc<Mutex<AppState>>,
    _session: Arc<launcher_core::SearchSession>,
    ui_weak: slint::Weak<AppWindow>,
    limit: usize,
) {
    std::thread::spawn(move || {
        let snapshot_mode = std::env::var("LAUNCHER_SNAPSHOT_DIR")
            .map(|v| !v.is_empty())
            .unwrap_or(false);
        {
            let mut st = state.lock().expect("state lock");
            st.current_results = st.core.recent_commands(limit);
            st.results_gen = st.context_gen;
            st.selected = 0;
        }
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = ui_weak.upgrade() {
                // rebuild on the UI thread (ResultItem carries slint::Image)
                let items = {
                    let st = state.lock().expect("state lock");
                    launcher_ui::to_result_items(st.current_results.iter().cloned(), 50)
                };
                ui.set_results(slint::ModelRc::new(slint::VecModel::from(items)));
            }
            // P2.1-E5: icons fill in asynchronously after publish
            spawn_icon_refresh(state, ui_weak, snapshot_mode);
        });
    });
}

fn spawn_search(
    state: Arc<Mutex<AppState>>,
    session: Arc<launcher_core::SearchSession>,
    ui_weak: slint::Weak<AppWindow>,
    query: String,
    limit: usize,
) {
    // File/IO and plugin work stays off the UI thread (spec: UI thread rules).
    std::thread::spawn(move || {
        // start a new generation; only the newest query may touch the UI
        let query_id = session.begin();
        // Main.Searching (UI-CONTRACT section 3.1): subtle, non-blocking
        let _ = slint::invoke_from_event_loop({
            let ui_weak = ui_weak.clone();
            move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_status("⌛ Searching".into());
                }
            }
        });
        let items = {
            let mut st = state.lock().expect("state lock");
            let results_gen = st.context_gen; // bind results to current context
            let r = st.core.search(&query, limit.min(launcher_core::MAX_RESULTS));
            if !session.is_current(query_id) {
                tracing::debug!(query_id, "query superseded, dropping results");
                return;
            }
            st.current_results = r.commands;
            st.results_gen = results_gen;
            st.selected = 0;
            // Main.Error vs Main.Empty (UI-CONTRACT section 3.1): provider
            // failures are never silently presented as "no results"
            let search_error = if st.current_results.is_empty() && !r.errors.is_empty() {
                Some(format!("⚠ Search failed: {}", r.errors.join("; ")))
            } else {
                None
            };
            let snapshot_mode = std::env::var("LAUNCHER_SNAPSHOT_DIR")
                .map(|v| !v.is_empty())
                .unwrap_or(false);
            (search_error, snapshot_mode)
        };
        let (search_error, snapshot_mode) = items;
        if !session.is_current(query_id) {
            return;
        }
        let _ = slint::invoke_from_event_loop(move || {
            if !session.is_current(query_id) {
                return; // a newer keystroke already won (ADR-0004)
            }
            if let Some(ui) = ui_weak.upgrade() {
                // rebuild on the UI thread (ResultItem carries slint::Image)
                let items = {
                    let st = state.lock().expect("state lock");
                    launcher_ui::to_result_items(st.current_results.iter().cloned(), 50)
                };
                ui.set_results(slint::ModelRc::new(std::rc::Rc::new(
                    slint::VecModel::from(items),
                )));
                ui.set_selected_index(0);
                ui.set_panel_visible(false);
                ui.set_actions(slint::ModelRc::new(std::rc::Rc::new(
                    slint::VecModel::from(Vec::<launcher_ui::ActionItem>::new()),
                )));
                ui.set_status(search_error.unwrap_or_default().into());
            }
            // P2.1-E5: icons fill in asynchronously after publish
            spawn_icon_refresh(state, ui_weak.clone(), snapshot_mode);
        });
    });
}

/// Context refresh (MVP2.1): must be called BEFORE the popup takes focus.
/// Captures the pre-popup foreground, resolves the Explorer folder, updates
/// the ContextProvider (Context Suggestions + context commands) and pushes
/// the suggestion list to the UI. Read-only, best-effort (INV-011).
fn spawn_context_refresh(
    state: Arc<Mutex<AppState>>,
    context_handle: launcher_core::providers::context::ContextHandle,
    ui_weak: slint::Weak<AppWindow>,
    session: Arc<launcher_core::SearchSession>,
    fg_hwnd: isize,
    fg_app: Option<String>,
    query_id: u64,
) {
    std::thread::spawn(move || {
        {
            let mut st = state.lock().expect("state lock");
            st.prev_foreground = Some(fg_hwnd);
            // new popup session -> new context generation (MVP3.2-C)
            st.context_gen += 1;
            st.results_gen = st.context_gen;
            st.pending_confirmation = None;
        }
        let folder = if fg_app.as_deref() == Some("explorer.exe") {
            launcher_context::explorer::folder_for_foreground(fg_hwnd)
        } else {
            None
        };
        let snap = ContextEngine::new(WindowsForegroundSource::new()).snapshot(folder.clone());
        tracing::info!(
            foreground = ?snap.foreground_app,
            folder = ?snap.current_folder,
            "context.snapshot"
        );
        // Context hint (UI-CONTRACT section 10): user-readable presentation
        // derived by Core data, never raw ContextSnapshot fields
        let context_hint = match (&snap.current_folder, &snap.foreground_app) {
            (Some(f), _) => {
                let name = std::path::Path::new(f)
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_else(|| f.clone());
                format!("📁 {name}")
            }
            (None, Some(app)) => format!("✦ {app}"),
            _ => String::new(),
        };

        // context commands + Context Suggestions file list (bounded)
        let mut commands = launcher_core::context_commands(&snap);
        if let Some(folder) = &snap.current_folder {
            let entries = std::fs::read_dir(folder)
                .map(|rd| {
                    rd.flatten()
                        .take(launcher_core::providers::context::MAX_CONTEXT_SUGGESTIONS)
                        .filter_map(|e| {
                            let path = e.path();
                            let name = path.file_name()?.to_string_lossy().to_string();
                            if name.starts_with('.') {
                                return None;
                            }
                            Some((name, path.display().to_string(), path.is_dir()))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            for (name, path, is_dir) in entries {
                commands.push(launcher_domain::Command {
                    id: format!("qs:{path}"),
                    title: name,
                    subtitle: Some(path.clone()),
                    icon: None,
                    provider_id: "context".into(),
                    score: 0.0,
                    keywords: vec![],
                    category: if is_dir {
                        launcher_domain::Category::Folder
                    } else {
                        launcher_domain::Category::File
                    },
                    actions: vec![launcher_domain::Action {
                        kind: launcher_domain::ActionKind::Open,
                        payload: Some(launcher_domain::ActionPayload::Path(path)),

                        id: None,
                        title: None,
                        disabled_reason: None,
                        shortcut: None,
                        confirmation_required: false,
                    }],
                    target: None,
                });
            }
        }
        context_handle.replace(commands.clone());

        // Context Suggestions (formerly Quick Switch): push the list for the
        // empty query on this popup open. The same Vec backs both the render
        // and the id lookup, so Enter always executes what is displayed.
        let _ = slint::invoke_from_event_loop({
            let ui_weak = ui_weak.clone();
            let hint = context_hint.clone();
            move || {
                if let Some(ui) = ui_weak.upgrade() {
                    ui.set_context_hint(hint.into());
                }
            }
        });
        if !commands.is_empty() && session.is_current(query_id) {
            if let Ok(mut st) = state.try_lock() {
                st.current_results = commands.clone();
                st.selected = 0;
            }
            let _ = slint::invoke_from_event_loop(move || {
                if session.is_current(query_id) {
                    if let Some(ui) = ui_weak.upgrade() {
                        // rebuild on the UI thread (ResultItem carries Image)
                        let items =
                            launcher_ui::to_result_items(commands.iter().cloned(), 50);
                        ui.set_results(slint::ModelRc::new(std::rc::Rc::new(
                            slint::VecModel::from(items),
                        )));
                        ui.set_selected_index(0);
                        ui.set_panel_visible(false);
                    }
                }
            });
        }
    });
}

/// Rebuild the Action Panel model for the currently selected command
/// (MVP3.1). The UI only receives ActionPresentation projections (INV-035/036).
fn sync_action_panel(state: &Arc<Mutex<AppState>>, ui_weak: &slint::Weak<AppWindow>) {
    let ui_weak = ui_weak.clone();
    let items = {
        let st = state.lock().expect("state lock");
        st.current_results
            .get(st.selected)
            .map(launcher_ui::to_action_presentations)
            .unwrap_or_default()
    };
    tracing::debug!(
        titles = ?items.iter().map(|a| (a.title.clone(), a.enabled)).collect::<Vec<_>>(),
        "action_panel.sync"
    );
    let items: Vec<launcher_ui::ActionItem> = items
        .into_iter()
        .map(|a| launcher_ui::ActionItem {
            action_id: a.action_id.into(),
            title: a.title.into(),
            enabled: a.enabled,
            reason: a.reason.into(),
            shortcut: a.shortcut.into(),
            is_primary: a.is_primary,
        })
        .collect();
    // ConfirmationPending substate (UI-CONTRACT section 12): true only while
    // the pending confirmation belongs to the command currently displayed
    let (confirmation_pending, _selected_command_id) = {
        let st = state.lock().expect("state lock");
        let cmd = st.current_results.get(st.selected).map(|c| c.id.clone());
        let pending = st
            .pending_confirmation
            .as_ref()
            .map(|(c, _)| Some(c.clone()) == cmd)
            .unwrap_or(false);
        (pending, cmd)
    };
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_actions(slint::ModelRc::new(std::rc::Rc::new(
                slint::VecModel::from(items),
            )));
            ui.set_panel_selected(0);
            ui.set_confirmation_pending(confirmation_pending);
        }
    });
}

/// Show a transient execution error in the popup (MVP3.1 result feedback).
fn set_status(ui_weak: slint::Weak<AppWindow>, msg: String) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            ui.set_status(msg.into());
        }
    });
}

/// Close the popup (park) and return focus to the pre-popup foreground app.
fn close_and_restore(
    ui_weak: slint::Weak<AppWindow>,
    visible: Arc<AtomicBool>,
    prev: Option<isize>,
) {
    let _ = slint::invoke_from_event_loop(move || {
        if let Some(ui) = ui_weak.upgrade() {
            foreground::park(ui.window());
            visible.store(false, Ordering::SeqCst);
        }
        #[cfg(windows)]
        if let Some(hwnd) = prev {
            foreground::restore_foreground(hwnd);
        }
    });
}

/// Context staleness guard (MVP3.2-C, INV-043/045): results resolved under
/// an older context generation MUST NOT silently execute.
fn context_is_stale(state: &Arc<Mutex<AppState>>) -> bool {
    let st = state.lock().expect("state lock");
    st.results_gen != st.context_gen
}

// ---- MUST-2: settings command + config hot-reload ----------------------

/// "Open Settings" command: surfaces the config file as a searchable result
/// (previously the only way to change settings was hand-editing a TOML file
/// with no discoverable entry point).
struct SettingsProvider {
    cmd: Command,
}

impl SettingsProvider {
    fn new(config_path: &std::path::Path) -> Self {
        Self {
            cmd: Command {
                id: "open-settings".into(),
                title: "Open Settings".into(),
                subtitle: Some(config_path.display().to_string()),
                icon: None,
                provider_id: "settings".into(),
                score: 0.0,
                keywords: vec!["settings".into(), "config".into()],
                category: launcher_domain::Category::Command,
                actions: vec![launcher_domain::Action {
                    kind: launcher_domain::ActionKind::Open,
                    payload: Some(launcher_domain::ActionPayload::Path(
                        config_path.display().to_string(),
                    )),
                    id: Some("open".into()),
                    title: Some("Edit config".into()),
                    disabled_reason: None,
                    shortcut: None,
                    confirmation_required: false,
                }],
                target: Some(config_path.display().to_string()),
            },
        }
    }
}

impl launcher_core::Provider for SettingsProvider {
    fn id(&self) -> &str {
        "settings"
    }
    fn query(&mut self, _q: &launcher_domain::QueryContext) -> Vec<Command> {
        vec![self.cmd.clone()]
    }
}

/// Everything the hotkey listener needs, so registration can be repeated
/// when the user changes the hotkey in the config file.
#[derive(Clone)]
struct HotkeyDeps {
    ui_weak: slint::Weak<AppWindow>,
    visible: Arc<AtomicBool>,
    ever_shown: Arc<AtomicBool>,
    session: Arc<launcher_core::SearchSession>,
    context_handle: launcher_core::providers::context::ContextHandle,
    state: Arc<Mutex<AppState>>,
    /// Live guards; dropping a guard unregisters its hotkey (MUST-2 reload).
    guards: Arc<Mutex<Vec<GlobalHotkey>>>,
    config_path: PathBuf,
    reload_state: Arc<Mutex<ReloadState>>,
    /// GA-2 bench collector; None unless LAUNCHER_HOTKEY_BENCH is set.
    bench: Option<(Arc<HotkeyBench>, PathBuf, usize)>,
}

#[derive(Debug, Clone)]
struct ReloadState {
    mtime: Option<std::time::SystemTime>,
    hotkey: String,
    result_limit: usize,
}

/// GA-2 hotkey latency bench (Test Plan §8.1): when LAUNCHER_HOTKEY_BENCH is
/// set, synthetic hotkey toggles drive the real dispatch→show pipeline and
/// per-show "dispatch→ready" samples are aggregated into a P50/P95 report.
#[derive(Default)]
struct HotkeyBench {
    samples_us: Mutex<Vec<u64>>,
}

impl HotkeyBench {
    fn record(&self, ready_us: u64) {
        self.samples_us.lock().expect("bench samples").push(ready_us);
    }

    fn write_report(&self, report: &Path, cycles_requested: usize) {
        let mut s = self.samples_us.lock().expect("bench samples").clone();
        s.sort_unstable();
        let pct = |p: usize| s.get(s.len() * p / 100).copied().unwrap_or(0);
        let body = serde_json::json!({
            "cycles_requested": cycles_requested,
            "show_samples": s.len(),
            "p50_us": pct(50),
            "p95_us": pct(95),
            "target_us": { "p50": 20_000, "p95": 35_000 },
            "scope": "WM_HOTKEY dispatch -> popup shown + recentered (physical key -> OS dispatch is out of process scope)",
        });
        if let Err(e) = std::fs::write(report, serde_json::to_string_pretty(&body).unwrap()) {
            tracing::error!(error = %e, path = %report.display(), "hotkey bench report write failed");
        } else {
            tracing::info!(path = %report.display(), p50_us = pct(50), p95_us = pct(95), samples = s.len(), "hotkey.bench.report");
        }
    }
}

/// Re-read the config when its mtime changed since the last load (MUST-2):
/// re-applies accent color / UI scale, and re-registers the global hotkey if
/// it changed. Called on every popup show, so the settings loop is:
/// edit config in the editor -> save -> recall the launcher -> applied.
fn maybe_reload_config(deps: &HotkeyDeps, ui: &AppWindow) {
    let mtime = std::fs::metadata(&deps.config_path)
        .and_then(|m| m.modified())
        .ok();
    {
        let rs = deps.reload_state.lock().expect("reload state");
        if rs.mtime == mtime {
            return;
        }
    }
    let cfg = match launcher_config::load_or_create(&deps.config_path) {
        Ok(c) => c,
        Err(e) => {
            tracing::warn!(error = %e, "config reload failed");
            return;
        }
    };
    ui.set_accent(parse_theme_color(&cfg.theme_color));
    apply_ui_scale(ui);
    let hotkey_changed = {
        let mut rs = deps.reload_state.lock().expect("reload state");
        let changed = rs.hotkey != cfg.hotkey;
        *rs = ReloadState {
            mtime,
            hotkey: cfg.hotkey.clone(),
            result_limit: cfg.result_limit,
        };
        changed
    };
    if hotkey_changed {
        // dropping the old guards unregisters the old hotkey(s)
        deps.guards.lock().expect("hotkey guards").clear();
        register_hotkey(&cfg.hotkey, deps);
        info!(hotkey = %cfg.hotkey, "config.hotkey_reregistered");
    }
    info!(result_limit = cfg.result_limit, "config.reloaded");
}

fn register_hotkey(hotkey_str: &str, deps: &HotkeyDeps) {
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    let (result_tx, result_rx) =
        std::sync::mpsc::channel::<Result<(), launcher_hotkey::HotkeyError>>();
    match launcher_hotkey::parse_hotkey(hotkey_str) {
        Ok(parsed) => {
            let guard = GlobalHotkey::spawn(parsed, tx.clone(), result_tx);
            deps.guards.lock().expect("hotkey guards").push(guard);
        }
        Err(e) => {
            tracing::error!(error = %e, "cannot parse configured hotkey, hotkey disabled");
        }
    }
    // GA-2 bench driver: synthetic toggles through the real receive loop.
    // Started once even if the hotkey is re-registered by config reload.
    if let Some((bench, report, cycles)) = deps.bench.clone() {
        static BENCH_STARTED: std::sync::atomic::AtomicBool =
            std::sync::atomic::AtomicBool::new(false);
        if !BENCH_STARTED.swap(true, Ordering::SeqCst) {
            std::thread::Builder::new()
                .name("hotkey-bench".into())
                .spawn(move || {
                    for _ in 0..cycles {
                        let _ = tx.send(());
                        std::thread::sleep(std::time::Duration::from_millis(25));
                    }
                    // wait for the show half of the toggles to be processed
                    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
                    while bench.samples_us.lock().expect("bench samples").len() * 2 < cycles
                        && std::time::Instant::now() < deadline
                    {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                    bench.write_report(&report, cycles);
                    let _ = slint::invoke_from_event_loop(|| {
                        let _ = slint::quit_event_loop();
                    });
                })
                .ok();
        }
    }
    let err_str = hotkey_str.to_string();
    std::thread::Builder::new()
        .name("hotkey-result".into())
        .spawn(move || {
            if let Ok(Err(e)) = result_rx.recv() {
                tracing::error!(error = %e, hotkey = %err_str, "hotkey registration failed");
            }
        })
        .ok();
    let deps = deps.clone();
    std::thread::spawn(move || {
        while let Ok(()) = rx.recv() {
            // capture pre-popup foreground BEFORE the popup takes focus
            let fg_hwnd = launcher_context::windows_source::foreground_hwnd();
            let fg_app = WindowsForegroundSource::new().foreground_app();
            tracing::debug!(fg_hwnd, ?fg_app, "hotkey.prepopup.capture");
            let query_id = deps.session.begin();
            // T1 (docs/PERFORMANCE-CONTRACT.md): hotkey dispatch ->
            // show request; T0 (physical key -> OS dispatch) is outside
            // process scope.
            let dispatch_at = std::time::Instant::now();
            let deps = deps.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = deps.ui_weak.upgrade() {
                    if deps.visible.load(Ordering::SeqCst) {
                        foreground::park(ui.window());
                        deps.visible.store(false, Ordering::SeqCst);
                        #[cfg(windows)]
                        if let Some(hwnd) =
                            deps.state.lock().ok().and_then(|st| st.prev_foreground)
                        {
                            foreground::restore_foreground(hwnd);
                        }
                    } else {
                        let t1_us = dispatch_at.elapsed().as_micros() as u64;
                        let cold = !deps.ever_shown.swap(true, Ordering::SeqCst);
                        let show_started = std::time::Instant::now();
                        let _ = ui.show();
                        deps.visible.store(true, Ordering::SeqCst);
                        let dispatch_to_ready_us = dispatch_at.elapsed().as_micros() as u64;
                        tracing::info!(
                            cold,
                            t1_dispatch_to_show_us = t1_us,
                            t2_show_call_us = show_started.elapsed().as_micros() as u64,
                            dispatch_to_ready_us,
                            "popup.latency"
                        );
                        if let Some((bench, _, _)) = &deps.bench {
                            bench.record(dispatch_to_ready_us);
                        }
                        #[cfg(windows)]
                        foreground::take_foreground("Launcher");
                        foreground::recenter_and_repaint(ui.window());
                        win_platform::apply_tool_window(ui.window());
                        apply_ui_scale(&ui); // first show: winit DPI now known
                        // MUST-2: pick up config edits made since last show
                        maybe_reload_config(&deps, &ui);
                        // P2.2-C: refresh the per-popup context snapshot
                        {
                            let folder = launcher_context::explorer::explorer_folders()
                                .into_iter()
                                .find(|(h, _)| *h == fg_hwnd)
                                .map(|(_, f)| f);
                            deps.state.lock().ok().map(|st| {
                                st.core.set_search_context(Some(
                                    launcher_search::ContextSnapshot {
                                        foreground_app: fg_app.clone(),
                                        current_folder: folder,
                                    },
                                ));
                            });
                        }
                        ui.invoke_focus_input(); // keyboard focus lands in the search input
                        spawn_context_refresh(
                            deps.state.clone(),
                            deps.context_handle.clone(),
                            deps.ui_weak.clone(),
                            deps.session.clone(),
                            fg_hwnd,
                            fg_app,
                            query_id,
                        );
                        // fresh popup: clear stale query text; the Quick
                        // Switch list arrives from the context refresh
                        ui.invoke_reset_query();
                    }
                }
            });
        }
    });
}

// ---- MUST-1: installed-workflow command routing -------------------------

/// Route a `workflows` command to the WorkflowRunner (host-owned namespace
/// routing, same pattern as `mcp:` / `plugin:`; the engine never opens a
/// workflow file as a document).
fn run_installed_workflow(
    state: Arc<Mutex<AppState>>,
    ui_weak: slint::Weak<AppWindow>,
    file: String,
) {
    match launcher_core::providers::workflows::load_definition(std::path::Path::new(&file)) {
        Ok(def) => {
            info!(workflow = %def.name, "workflow.triggered");
            let state2 = state;
            let _ = slint::invoke_from_event_loop(move || {
                crate::workflow_service::start_workflow(state2, ui_weak.clone(), def);
                if let Some(ui) = ui_weak.upgrade() {
                    let _ = ui.show();
                    ui.set_wf_visible(true);
                    ui.invoke_focus_keys();
                }
            });
        }
        Err(e) => {
            tracing::warn!(file = %file, error = %e, "workflow definition invalid");
            set_status(ui_weak, format!("⚠ invalid workflow: {e}"));
        }
    }
}
/// Execute one action of one command by stable ids (shared by primary Enter,
/// Action Panel selection and shortcut dispatch — all paths are identical
/// below the dispatch, per INV-037/040).
#[allow(clippy::too_many_arguments)]
fn execute_action_by_id(
    state: Arc<Mutex<AppState>>,
    ui_weak: slint::Weak<AppWindow>,
    visible: Arc<AtomicBool>,
    command_id: String,
    action_id: String,
    primary: bool,
) {
    if context_is_stale(&state) {
        tracing::warn!(command = %command_id, "execute.stale_context");
        set_status(ui_weak, "⚠ Context changed - reopen the launcher".into());
        return;
    }
    // MUST-1: installed-workflow commands route by the host-owned
    // `workflows` provider namespace into the WorkflowRunner — the engine
    // never sees a workflow file as an Open target (same pattern as mcp:/plugin:).
    let (routed_provider, routed_target) = {
        let st = state.lock().expect("state lock");
        st.current_results
            .iter()
            .find(|c| c.id == command_id)
            .filter(|c| c.provider_id == "workflows" || c.provider_id == "settings")
            .map(|c| (c.provider_id.clone(), c.target.clone()))
            .unwrap_or_default()
    };
    match (routed_provider.as_str(), routed_target) {
        ("workflows", Some(file)) => {
            run_installed_workflow(state, ui_weak, file);
            return;
        }
        ("settings", Some(path)) => {
            // open the config in the default editor (notepad fallback)
            fn open_with_default_editor(path: &str) -> std::io::Result<()> {
                if std::process::Command::new("notepad").arg(path).spawn().is_ok() {
                    return Ok(());
                }
                #[cfg(windows)]
                {
                    use windows::core::HSTRING;
                    use windows::Win32::UI::Shell::ShellExecuteW;
                    use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;
                    let r = unsafe {
                        ShellExecuteW(
                            None,
                            &HSTRING::from("open"),
                            &HSTRING::from(path),
                            None,
                            None,
                            SW_SHOWNORMAL,
                        )
                    };
                    if r.0 as isize > 32 {
                        return Ok(());
                    }
                }
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    "no editor available",
                ))
            }
            let opened = open_with_default_editor(&path);
            match opened {
                Ok(_) => {
                    let prev = state.lock().ok().and_then(|st| st.prev_foreground);
                    close_and_restore(ui_weak.clone(), visible.clone(), prev);
                }
                Err(e) => set_status(ui_weak, format!("⚠ cannot open editor: {e}")),
            }
            return;
        }
        _ => {}
    }
    let resolved = {
        let st = state.lock().expect("state lock");
        st.current_results
            .iter()
            .find(|c| c.id == command_id)
            .and_then(|c| {
                c.actions
                    .iter()
                    .find(|a| a.id.as_deref() == Some(action_id.as_str()))
                    .cloned()
            })
            .map(|mut a| {
                // system.paste carries its target: the pre-popup foreground the
                // engine restores before sending the keystroke (MVP3.2)
                if a.kind == launcher_domain::ActionKind::Paste && a.payload.is_none() {
                    a.payload = Some(launcher_domain::ActionPayload::Hwnd(
                        st.prev_foreground.unwrap_or(0) as i64,
                    ));
                }
                a
            })
    };
    let Some(mut action) = resolved else { return };
    info!(command = %command_id, action = %action_id, primary, "action.execute");

    // Confirmation policy (MVP3.2-B, INV-041/042): the FIRST attempt arms
    // the pending state; the SECOND Enter on the same action is the host's
    // confirmation and clears the policy flag before the engine runs.
    if action.confirmation_required {
        let mut st = state.lock().expect("state lock");
        if st.pending_confirmation.as_ref() != Some(&(command_id.clone(), action_id.clone())) {
            st.pending_confirmation = Some((command_id.clone(), action_id.clone()));
            drop(st);
            set_status(ui_weak, "⚠ Confirm: press Enter again".into());
            return;
        }
        st.pending_confirmation = None;
        action.confirmation_required = false;
    }

    std::thread::spawn(move || {
        // FIX-02 (review 68 §12): every attempt carries one execution_id —
        // system effects get theirs here (plugin/mcp ids flow from Core).
        let execution_id = {
            let st = state.lock().expect("state lock");
            st.core.next_execution_id()
        };
        tracing::info!(execution_id = %execution_id, command = %command_id, "attempt.begin");
        match launcher_action::execute(&action) {
            Ok(Effect::Launched(_) | Effect::Copied(_) | Effect::Revealed(_) | Effect::Pasted) => {
                // P2-D usage history: feed the bounded history sink so
                // ranking can boost frequently/recently used commands.
                {
                    let mut st = state.lock().expect("state lock");
                    if let Some(c) = st.command_for_id(&command_id) {
                        st.core
                            .record_use_with_title(&command_id, &c.provider_id, &c.title);
                    }
                }
                let prev = state.lock().ok().and_then(|st| st.prev_foreground);
                close_and_restore(ui_weak.clone(), visible.clone(), prev);
            }
            // MVP4.0/4.3 (ADR-0014): engine-validated plugin/mcp action ->
            // route through the Core effect registry, which picks the
            // executor by the host-assigned provider_id namespace only.
            Ok(Effect::PluginInvoked { action_id, input }) => {
                // namespace comes from the host-assigned provider_id
                // ("mcp:<server>" / "plugin:<manifest.id>"), never from
                // producer-controlled data
                let provider_id = state
                    .lock()
                    .ok()
                    .and_then(|st| {
                        st.current_results
                            .iter()
                            .find(|c| c.id == command_id)
                            .map(|c| c.provider_id.clone())
                    })
                    .unwrap_or_default();
                let gen = state.lock().map(|st| st.context_gen).unwrap_or(0);
                let outcome = state.lock().ok().and_then(|mut st| {
                    let execution_id = st.core.next_execution_id();
                    st.core
                        .execute_effect(
                            &provider_id,
                            &action_id.unwrap_or_default(),
                            &input,
                            &execution_id,
                            gen,
                        )
                        .ok()
                });
                match outcome {
                    Some(result) => {
                        tracing::info!(provider = %provider_id, result = %result, "effect.done");
                        if let Some(c) =
                            state.lock().ok().and_then(|st| st.command_for_id(&command_id))
                        {
                            let title = c.title.clone();
                            state.lock().ok().map(|mut st| {
                                st.core.record_use_with_title(&command_id, &c.provider_id, &title)
                            });
                        }
                        close_and_restore(ui_weak.clone(), visible.clone(), None);
                    }
                    None => {
                        tracing::warn!(provider = %provider_id, "effect.failed");
                        set_status(ui_weak, format!("⚠ action failed: {provider_id}"));
                    }
                }
            }
            Ok(_) => {}
            // committed execution: Esc does not cancel an in-flight effect
            Err(e) => {
                tracing::warn!(error = %e, "action.failed");
                set_status(ui_weak, format!("⚠ {e}"));
            }
        }
    });
}

fn spawn_execute(
    state: Arc<Mutex<AppState>>,
    ui_weak: slint::Weak<AppWindow>,
    visible: Arc<AtomicBool>,
    command_id: String,
) {
    // Primary Enter = the first Ready action, dispatched through the exact
    // same id-based path as the Action Panel (INV-037/040).
    let action_id = {
        let st = state.lock().expect("state lock");
        st.command_for_id(&command_id)
            .and_then(|c| c.primary_action().and_then(|a| a.id.clone()))
    };
    match action_id {
        Some(id) => execute_action_by_id(state, ui_weak, visible, command_id, id, true),
        // informational command: Enter does nothing, no error
        None => tracing::debug!(command = %command_id, "informational command, no primary"),
    }
}

#[cfg(windows)]
mod foreground {
    /// Bring the launcher window to the foreground after show().
    ///
    /// Windows denies foreground activation to background processes; the
    /// WM_HOTKEY path in this process grants us the right, but the show runs
    /// on the UI thread while the hotkey arrives on the hotkey thread, so we
    /// re-assert foreground explicitly. This is the single justified `unsafe`
    /// block in the MVP (architecture red line: documented, reviewed).
    /// Re-show hardening (hide->show cycles): the backend can leave the
    /// window off-screen or without a fresh surface paint, which looks like
    /// "hotkey did nothing" even though the OS reports visible+foreground.
    /// Recenter on the primary monitor and force a redraw.
    pub fn recenter_and_repaint(window: &slint::Window) {
        #[cfg(windows)]
        {
            use windows::Win32::Foundation::POINT;
            use windows::Win32::Graphics::Gdi::{
                GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
            };
            use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
            // P2-C: place the popup on the monitor that owns the CURSOR
            // (falls back to the primary monitor metrics when enumeration
            // fails), so the launcher appears where the user invoked it.
            let mut monitor_rect: Option<(i32, i32, i32, i32)> = None;
            unsafe {
                let mut pt = POINT::default();
                if GetCursorPos(&mut pt).is_ok() {
                    let hmon = MonitorFromPoint(pt, MONITOR_DEFAULTTONEAREST);
                    let mut info = MONITORINFO {
                        cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                        ..Default::default()
                    };
                    if GetMonitorInfoW(hmon, &mut info).as_bool() {
                        let rc = info.rcWork;
                        monitor_rect = Some((rc.left, rc.top, rc.right, rc.bottom));
                    }
                }
            }
            let (mx, my, mw, mh) = match monitor_rect {
                Some((l, t, r, b)) => (l, t, (r - l).max(1), (b - t).max(1)),
                None => unsafe {
                    (
                        0,
                        0,
                        GetSystemMetrics(SM_CXSCREEN).max(1),
                        GetSystemMetrics(SM_CYSCREEN).max(1),
                    )
                },
            };
            let size = window.size();
            window.set_position(slint::PhysicalPosition::new(
                mx + ((mw - size.width as i32) / 2).max(0),
                my + ((mh - size.height as i32) / 3).max(0),
            ));
        }
        window.request_redraw();
    }

    /// "Hide" the popup WITHOUT destroying/hiding the native window: Slint's
    /// hide() (winit) leaves the window iconic with a dead surface after a
    /// show/hide cycle (reproduced: IsIconic=true, OS-visible but blank).
    /// Parking it off-screen keeps the window mapped, so every show is just a
    /// move + repaint of a live window.
    pub fn park(window: &slint::Window) {
        window.set_position(slint::PhysicalPosition::new(-32000, -32000));
        // release keyboard focus so the off-screen window does not eat keys
        #[cfg(windows)]
        unsafe {
            let _ = windows::Win32::UI::Input::KeyboardAndMouse::SetFocus(None);
        }
    }

    /// Give focus back to the window that was foreground before the popup
    /// opened (focus return on every popup close path, MVP3.1).
    pub fn restore_foreground(hwnd: isize) {
        use windows::Win32::Foundation::HWND;
        use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
        use windows::Win32::UI::WindowsAndMessaging::{
            IsIconic, SetForegroundWindow, ShowWindow, SW_RESTORE,
        };
        if hwnd == 0 {
            return;
        }
        let hwnd = HWND(hwnd as *mut _);
        unsafe {
            if IsIconic(hwnd).as_bool() {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            }
            let ok = SetForegroundWindow(hwnd).as_bool();
            let _ = SetFocus(hwnd);
            tracing::debug!(set_foreground = ok, "foreground.restore");
        }
    }

    pub fn take_foreground(title: &str) {
        use windows::core::HSTRING;
        use windows::Win32::Foundation::HWND;
        use windows::Win32::System::Threading::AttachThreadInput;
        use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
        use windows::Win32::UI::WindowsAndMessaging::{
            FindWindowW, GetForegroundWindow, GetWindowThreadProcessId, IsIconic, IsWindowVisible,
            SetForegroundWindow, SetWindowPos, ShowWindow, HWND_TOPMOST, SWP_NOMOVE, SWP_NOSIZE,
            SWP_SHOWWINDOW, SW_RESTORE, SW_SHOW,
        };
        unsafe {
            let Ok(hwnd) = FindWindowW(None, &HSTRING::from(title)) else {
                tracing::warn!("foreground.window_not_found");
                return;
            };
            let was_visible = IsWindowVisible(hwnd).as_bool();
            let minimized = IsIconic(hwnd).as_bool();
            // SW_SHOW does not restore a minimized window; a re-shown popup can
            // end up iconic (taskbar button, no visible frame) after a
            // hide/show cycle -> restore instead.
            if minimized {
                let _ = ShowWindow(hwnd, SW_RESTORE);
            } else {
                let _ = ShowWindow(hwnd, SW_SHOW);
            }
            // re-assert topmost: the always-on-top flag can be lost when a
            // hidden window is re-shown by the backend
            let _ = SetWindowPos(
                hwnd,
                HWND_TOPMOST,
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_SHOWWINDOW,
            );
            let ok = SetForegroundWindow(hwnd).as_bool();
            if !ok {
                // Standard fallback: attach our thread's input queue to the
                // current foreground thread so the system allows the switch.
                let fg: HWND = GetForegroundWindow();
                let fg_tid = GetWindowThreadProcessId(fg, None);
                let my_tid = windows::Win32::System::Threading::GetCurrentThreadId();
                let _ = AttachThreadInput(fg_tid, my_tid, true);
                let ok2 = SetForegroundWindow(hwnd).as_bool();
                let _ = AttachThreadInput(fg_tid, my_tid, false);
                tracing::debug!(attach_fallback = true, ok2, "foreground.retry");
            }
            let _ = SetFocus(hwnd);
            tracing::debug!(
                was_visible,
                minimized,
                set_foreground = ok,
                now_visible = IsWindowVisible(hwnd).as_bool(),
                "foreground.take"
            );
        }
    }
}

/// Crash diagnosability: panic location and message go to tracing (file logs).
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!(panic = %info, "process panicked");
        default_hook(info);
    }));
}

/// Logging: stdout layer + daily rolling file in `%APPDATA%\Launcher\logs`.
/// Level defaults to INFO; override with `LAUNCHER_LOG`.
fn init_logging() -> anyhow::Result<()> {
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_env("LAUNCHER_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let logs_dir = launcher_config::logs_dir()?;
    std::fs::create_dir_all(&logs_dir)?;
    let file_appender = tracing_appender::rolling::daily(logs_dir, "launcher.log");

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer().with_writer(std::io::stdout))
        .with(tracing_subscriber::fmt::layer().with_writer(std::sync::Mutex::new(file_appender)))
        .try_init()
        .map_err(|e| anyhow::anyhow!("logging already initialized: {e}"))?;
    Ok(())
}

/// P2-C: single instance via a named mutex held for the process lifetime.
/// A second launch exits silently - the resident instance owns the hotkey
/// and the tray, so a duplicate would only steal resources.
/// FIX-03 (review 68 §7): the mutex HANDLE must live for the WHOLE process,
/// not just the check function. The previous code dropped the handle when
/// the function returned — the kernel then destroyed the mutex object and a
/// second launch could create it again, so "single instance" never held.
pub struct SingleInstanceGuard {
    // held (never read) purely to keep the kernel mutex object alive —
    // closing the last handle destroys it and would allow a second instance
    #[cfg(windows)]
    #[allow(dead_code)]
    handle: windows::Win32::Foundation::HANDLE,
}

#[cfg(windows)]
fn enforce_single_instance() -> anyhow::Result<Option<SingleInstanceGuard>> {
    use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS};
    use windows::Win32::System::Threading::CreateMutexW;
    use windows::core::HSTRING;
    let name = HSTRING::from(r"Local\NativeLauncherSingleInstance");
    let handle = unsafe { CreateMutexW(None, false, &name)? };
    if unsafe { GetLastError() } == ERROR_ALREADY_EXISTS {
        return Ok(None); // local handle dropped; the FIRST instance owns the mutex
    }
    Ok(Some(SingleInstanceGuard { handle }))
}

#[cfg(not(windows))]
fn enforce_single_instance() -> anyhow::Result<Option<SingleInstanceGuard>> {
    Ok(Some(SingleInstanceGuard {}))
}

fn main() -> anyhow::Result<()> {
    install_panic_hook();
    init_logging()?;
    info!("launcher.started");
    // the guard is held until the end of main = process lifetime (FIX-03)
    let _instance_guard = match enforce_single_instance()? {
        Some(g) => g,
        None => {
            info!("launcher.already_running");
            return Ok(());
        }
    };

    let cfg_path = launcher_config::config_path()?;
    let cfg: AppConfig = launcher_config::load_or_create(&cfg_path)?;
    if cfg.autostart {
        autostart::set_autostart(true)?;
    }
    info!(hotkey = %cfg.hotkey, theme = %cfg.theme_color, autostart = cfg.autostart, "config loaded");

    let db_dir = data_dir();
    // P2.2-E startup health marker: detect a previous interrupted startup
    // (state stayed "starting" = the process died before reaching healthy).
    let startup_state_path = db_dir.join("startup_state.json");
    #[derive(serde::Serialize, serde::Deserialize)]
    struct StartupState {
        state: String,
        consecutive_failures: u32,
        version: String,
    }
    if let Ok(raw) = std::fs::read_to_string(&startup_state_path) {
        if let Ok(prev) = serde_json::from_str::<StartupState>(&raw) {
            if prev.state == "starting" {
                let failures = prev.consecutive_failures + 1;
                if failures >= 3 {
                    warn!(failures, "RECOVERY: repeated failed startups — booting degraded");
                }
                let _ = std::fs::write(
                    &startup_state_path,
                    serde_json::json!({"state":"starting","consecutive_failures":failures,"version":env!("CARGO_PKG_VERSION")}).to_string(),
                );
            }
        }
    } else {
        let _ = std::fs::write(
            &startup_state_path,
            serde_json::json!({"state":"starting","consecutive_failures":0,"version":env!("CARGO_PKG_VERSION")}).to_string(),
        );
    }
    let degraded_boot = {
        let mut degraded = false;
        if let Ok(raw) = std::fs::read_to_string(&startup_state_path) {
            if let Ok(prev) = serde_json::from_str::<serde_json::Value>(&raw) {
                if prev["state"] == "starting"
                    && prev["consecutive_failures"].as_u64().unwrap_or(0) >= 3
                {
                    degraded = true;
                }
            }
        }
        degraded
    };
    // P2.2-E §50 (WaitingForExit → recover handoff): consume any update
    // handoff left by the outgoing process. Content is upgrade metadata only
    // (§26) — never clipboard/query/credentials. Consume-once: whether this
    // boot reaches "healthy" is decided by the startup health marker above,
    // not by the handoff itself (§51: never guess upgrade success).
    if let Some(h) = launcher_config::handoff::consume_handoff(&db_dir) {
        if h.resume {
            info!(
                transaction = %h.transaction_id,
                target_version = %h.target_version,
                reason = %h.reason,
                "RECOVERY: interrupted upgrade handoff recovered — resuming normal boot under health check"
            );
        } else {
            info!(
                transaction = %h.transaction_id,
                target_version = %h.target_version,
                "update handoff consumed (no resume requested)"
            );
        }
    }
    let (core, context_handle) = build_core(&cfg, &db_dir, degraded_boot)?;

    let state = Arc::new(Mutex::new(AppState {
        core,
        icons: launcher_providers::icons::IconService::new(
            8 * 1024 * 1024, // INV-ICON-004: hard byte budget
            Some(db_dir.join("icon-cache")),
        ),
        selected: 0,
        prev_foreground: None,
        context_gen: 0,
        results_gen: 0,
        pending_confirmation: None,
        current_results: Vec::new(),
    }));
    let session = Arc::new(launcher_core::SearchSession::new());
    let visible = Arc::new(AtomicBool::new(false));
    let ever_shown = Arc::new(AtomicBool::new(false));

    let ui = AppWindow::new()?;
    // WIN-TRAY-001/002: the launcher popup is a transient tool window -
    // never a taskbar button (tray icon is the resident identity)
    win_platform::apply_tool_window(ui.window());
    apply_ui_scale(&ui);
    ui.set_accent(parse_theme_color(&cfg.theme_color));
    ui.set_results(slint::ModelRc::new(std::rc::Rc::new(
        slint::VecModel::from(Vec::<ResultItem>::new()),
    )));
    let ui_weak = ui.as_weak();

    // tray: resident + summon + quit (keeps the event loop alive when hidden)
    let tray = TrayIcon::new()?;
    tray.on_show_requested({
        let ui_weak = ui_weak.clone();
        let visible = visible.clone();
        let session = session.clone();
        let context_handle = context_handle.clone();
        let state = state.clone();
        move || {
            // capture pre-popup foreground (once popup takes focus it is the
            // foreground app itself)
            let fg_hwnd = launcher_context::windows_source::foreground_hwnd();
            let fg_app = WindowsForegroundSource::new().foreground_app();
            let query_id = session.begin();
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            let visible = visible.clone();
            let session = session.clone();
            let context_handle = context_handle.clone();
            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = ui_weak.upgrade() {
                    let _ = ui.show();
                    visible.store(true, Ordering::SeqCst);
                    #[cfg(windows)]
                    foreground::take_foreground("Launcher");
                    foreground::recenter_and_repaint(ui.window());
                            win_platform::apply_tool_window(ui.window());
                    apply_ui_scale(&ui); // first show: winit DPI now known
                    // P2.2-C: capture the search context ONCE per popup
                    {
                        let folder = launcher_context::explorer::explorer_folders()
                            .into_iter()
                            .find(|(h, _)| *h == fg_hwnd)
                            .map(|(_, f)| f);
                        state.lock().ok().map(|st| {
                            st.core.set_search_context(Some(
                                launcher_search::ContextSnapshot {
                                    foreground_app: fg_app.clone(),
                                    current_folder: folder,
                                },
                            ));
                        });
                    }
                    ui.invoke_focus_input(); // keyboard focus lands in the search input
                    spawn_context_refresh(
                        state,
                        context_handle,
                        ui_weak.clone(),
                        session,
                        fg_hwnd,
                        fg_app,
                        query_id,
                    );
                }
            });
        }
    });
    tray.on_quit_requested(|| {
        let _ = slint::quit_event_loop();
    });
    tray.on_workflow_demo_requested({
        let state = state.clone();
        let ui_weak = ui_weak.clone();
        move || {
            // trigger source: tray menu (trigger request only — the run is
            // created and executed by the workflow service / runner)
            info!("workflow.demo.triggered");
            crate::workflow_service::start_workflow(
                state.clone(),
                ui_weak.clone(),
                crate::workflow_service::demo_definition(),
            );
            // bring the launcher up so the runtime surface is visible
            if let Some(ui) = ui_weak.upgrade() {
                let _ = ui.show();
                ui.set_wf_visible(true);
                ui.invoke_focus_keys(); // workflow surface replaces the search box
            }
        }
    });

    // configurable global hotkey (MUST-2: re-registrable for hot reload)
    // GA-2: hotkey latency bench (Test Plan §8.1) — opt-in via env
    let hotkey_bench = std::env::var("LAUNCHER_HOTKEY_BENCH").ok().map(|p| {
        let cycles: usize = std::env::var("LAUNCHER_HOTKEY_BENCH_CYCLES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(200);
        (
            Arc::new(HotkeyBench::default()),
            PathBuf::from(p),
            cycles,
        )
    });
    let hotkey_deps = HotkeyDeps {
        ui_weak: ui_weak.clone(),
        visible: visible.clone(),
        ever_shown: ever_shown.clone(),
        session: session.clone(),
        context_handle: context_handle.clone(),
        state: state.clone(),
        guards: Arc::new(Mutex::new(Vec::new())),
        config_path: cfg_path.clone(),
        bench: hotkey_bench,
        reload_state: Arc::new(Mutex::new(ReloadState {
            mtime: std::fs::metadata(&cfg_path).and_then(|m| m.modified()).ok(),
            hotkey: cfg.hotkey.clone(),
            result_limit: cfg.result_limit,
        })),
    };
    register_hotkey(&cfg.hotkey, &hotkey_deps);

    // P2.2-A: Ctrl+D toggles favorite on the selected result
    {
        let state = state.clone();
        let ui_weak = ui_weak.clone();
        ui.on_favorite_toggle(move || {
            let selected = {
                let st = state.lock().expect("state lock");
                st.current_results.get(st.selected).cloned()
            };
            let Some(cmd) = selected else { return };
            let mut st = state.lock().expect("state lock");
            match st.core.toggle_favorite(&cmd) {
                Some(true) => set_status(ui_weak.clone(), format!("★ Favorite added")),
                Some(false) => set_status(ui_weak.clone(), format!("☆ Favorite removed")),
                None => set_status(ui_weak.clone(), format!("⚠ Favorites unavailable")),
            }
        });
    }
    {
        let state = state.clone();
        let session = session.clone();
        let ui_weak = ui_weak.clone();
        // MUST-2: read the LIVE result limit (updated by config hot reload)
        let reload_state = hotkey_deps.reload_state.clone();
        ui.on_query_changed(move |q| {
            let limit = reload_state
                .lock()
                .map(|rs| rs.result_limit)
                .unwrap_or(launcher_core::MAX_RESULTS);
            // P2.1 §57: empty query = the recent/frequent view, not "no results"
            spawn_search_or_recent(
                state.clone(),
                session.clone(),
                ui_weak.clone(),
                q.to_string(),
                limit,
            );
        });
    }
    {
        let state = state.clone();
        let ui_weak = ui_weak.clone();
        let visible = visible.clone();
        ui.on_execute(move |id| {
            spawn_execute(
                state.clone(),
                ui_weak.clone(),
                visible.clone(),
                id.to_string(),
            );
        });
    }
    {
        // MVP3.1 Action Panel callbacks (presentation + id-based execution)
        ui.on_selection_changed({
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            move |command_id| {
                // F6: the signal carries the business identity (command_id);
                // the index is resolved Rust-side and never crosses the
                // boundary as selection state
                let command_id = command_id.to_string();
                if let Ok(mut st) = state.lock() {
                    if let Some(i) = st.current_results.iter().position(|c| c.id == command_id) {
                        st.selected = i;
                    }
                    // UI-CONTRACT section 8.2: changing the selection cancels
                    // a pending confirmation - it never carries across commands
                    st.pending_confirmation = None;
                }
                sync_action_panel(&state, &ui_weak);
            }
        });
        ui.on_panel_toggled({
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            move || {
                sync_action_panel(&state, &ui_weak);
                // panel mode replaces the search box: move Slint item focus
                // to the root FocusScope so capture-phase keys keep firing
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_focus_keys();
                }
            }
        });
        // Workflow Runtime Surface events (UI-CONTRACT section 13): Confirm
        // resumes the paused run via the service channel; dismiss keeps the
        // run paused (Esc never cancels submitted effects)
        ui.on_workflow_confirm(move || crate::workflow_service::confirm_active_run());
        ui.on_workflow_dismiss({
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            move || {
                if let Ok(st) = state.lock() {
                    info!(
                        results_gen = st.results_gen,
                        "workflow.dismiss (kept paused)"
                    );
                }
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_focus_input();
                }
            }
        });
        // Esc in the panel = Cancel: a pending confirmation is never
        // persisted (UI-CONTRACT §12 / INV-048)
        ui.on_panel_closed({
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            move || {
                if let Ok(mut st) = state.lock() {
                    st.pending_confirmation = None;
                }
                if let Some(ui) = ui_weak.upgrade() {
                    ui.invoke_focus_input();
                }
            }
        });
        let ui_weak = ui_weak.clone();
        ui.on_panel_nav(move |delta| {
            if let Some(ui) = ui_weak.upgrade() {
                use slint::Model as _;
                let actions = ui.get_actions();
                let len = actions.row_count();
                let mut sel = ui.get_panel_selected() as i32;
                // skip disabled rows; they are presentable, not selectable
                loop {
                    sel += delta;
                    if sel < 0 || sel as usize >= len {
                        return;
                    }
                    if let Some(item) = actions.row_data(sel as usize) {
                        if item.enabled {
                            break;
                        }
                    }
                }
                ui.set_panel_selected(sel);
            }
        });
    }
    {
        let state = state.clone();
        let ui_weak = ui_weak.clone();
        let visible = visible.clone();
        ui.on_execute_action({
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            let visible = visible.clone();
            move |command_id, action_id| {
                execute_action_by_id(
                    state.clone(),
                    ui_weak.clone(),
                    visible.clone(),
                    command_id.to_string(),
                    action_id.to_string(),
                    false,
                );
            }
        });
        // MVP3.2-A: shortcut dispatch resolves to the stable action_id of the
        // selected command; collision rule = first declaration wins
        // (deterministic, independent of widget order — INV-039/040).
        ui.on_action_shortcut({
            let state = state.clone();
            let ui_weak = ui_weak.clone();
            let visible = visible.clone();
            move |ch| {
                let want = format!("Ctrl+Shift+{}", ch.to_uppercase());
                let target = {
                    let st = state.lock().expect("state lock");
                    st.current_results.get(st.selected).and_then(|c| {
                        c.actions
                            .iter()
                            .find(|a| {
                                a.disabled_reason.is_none()
                                    && a.shortcut.as_deref() == Some(want.as_str())
                            })
                            .and_then(|a| a.id.clone())
                    })
                };
                if let (Some(cmd), Some(action_id)) = (
                    state
                        .lock()
                        .ok()
                        .and_then(|st| st.current_results.get(st.selected).map(|c| c.id.clone())),
                    target,
                ) {
                    execute_action_by_id(
                        state.clone(),
                        ui_weak.clone(),
                        visible.clone(),
                        cmd,
                        action_id,
                        false,
                    );
                }
            }
        });
    }
    {
        let visible = visible.clone();
        let ui_weak = ui_weak.clone();
        ui.on_dismissed({
            let state = state.clone();
            move || {
                if let Ok(mut st) = state.lock() {
                    st.pending_confirmation = None;
                }
                if let Some(ui) = ui_weak.upgrade() {
                    foreground::park(ui.window());
                    visible.store(false, Ordering::SeqCst);
                }
                #[cfg(windows)]
                if let Some(hwnd) = state.lock().ok().and_then(|st| st.prev_foreground) {
                    foreground::restore_foreground(hwnd);
                }
            }
        });
    }


    // P2.2-E: all core services initialized — mark this startup healthy
    {
        let healthy = serde_json::json!({
            "state": "healthy",
            "consecutive_failures": 0,
            "version": env!("CARGO_PKG_VERSION"),
        });
        let _ = std::fs::write(&startup_state_path, healthy.to_string());
        info!(version = env!("CARGO_PKG_VERSION"), "startup healthy marker written");
    }

    tray.show()?;

    // P2.3-F keyboard walkthrough (LAUNCHER_KEYBOARD_WALKTHROUGH=<report>):
    // drive the real Slint key pipeline through the Main-mode ladder and
    // quit. Runs ahead of the snapshot branch so both can be combined.
    if let Some(report) = std::env::var("LAUNCHER_KEYBOARD_WALKTHROUGH")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        keyboard_walkthrough::run(ui.as_weak(), visible.clone(), report);
    }

    if let Some(report) = std::env::var("LAUNCHER_DPI_WALKTHROUGH")
        .ok()
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
    {
        keyboard_walkthrough::run_dpi(ui.as_weak(), report);
    }

    // Visual Regression baselines (Spec step 8 / section 51, review 39):
    // LAUNCHER_SNAPSHOT_DIR=<dir> captures the 10 frozen UI states
    // (VR-001..VR-010) as BMPs at 640x420 / Dark / 100% scale, then quits.
    if let Some(dir) = std::env::var("LAUNCHER_SNAPSHOT_DIR")
        .ok()
        .filter(|v| !v.is_empty())
    {
        visual_scenarios::capture_all(ui.as_weak(), dir.into());
    }
    // single-file legacy snapshot (LAUNCHER_SNAPSHOT=<path.bmp>)
    else if let Some(path) = snapshot::snapshot_requested() {
        snapshot::push_demo_data(&ui);
        let _ = ui.show();
        ui.invoke_focus_input();
        std::thread::spawn(move || {
            // allow the software renderer to paint the full frame
            std::thread::sleep(std::time::Duration::from_millis(900));
            let hwnd = snapshot::find_launcher_hwnd().unwrap_or(0);
            match snapshot::capture_client_to_bmp(hwnd) {
                Some(bmp) => {
                    if std::fs::write(&path, &bmp).is_ok() {
                        tracing::info!(?path, bytes = bmp.len(), "snapshot.written");
                    } else {
                        tracing::error!(?path, "snapshot.write_failed");
                    }
                }
                None => tracing::error!(?path, "snapshot.capture_failed"),
            }
            let _ = slint::quit_event_loop();
        });
    }

    // UI show/hide soak (docs/PERFORMANCE-CONTRACT.md P1 Stability):
    // LAUNCHER_SOAK_SHOWHIDE=N runs N visible-cycles with periodic memory
    // sampling, prints a trend report, then quits. Diagnostic-only path.
    if let Ok(n) = std::env::var("LAUNCHER_SOAK_SHOWHIDE") {
        let n: u32 = n.parse().unwrap_or(1000);
        let ui_weak = ui_weak.clone();
        let visible = visible.clone();
        std::thread::spawn(move || {
            let mut mem_samples: Vec<u64> = Vec::new();
            for i in 0..n {
                let _ = slint::invoke_from_event_loop({
                    let ui_weak = ui_weak.clone();
                    let visible = visible.clone();
                    move || {
                        if let Some(ui) = ui_weak.upgrade() {
                            let _ = ui.show();
                            foreground::park(ui.window());
                            visible.store(false, Ordering::SeqCst);
                        }
                    }
                });
                if i % 100 == 0 {
                    std::thread::sleep(std::time::Duration::from_millis(10));
                    let b = private_bytes();
                    if b > 0 {
                        mem_samples.push(b);
                    }
                }
            }
            std::thread::sleep(std::time::Duration::from_millis(300));
            let final_b = private_bytes();
            let initial = mem_samples.first().copied().unwrap_or(0);
            let peak = mem_samples.iter().copied().max().unwrap_or(0);
            tracing::info!(
                cycles = n,
                initial_private = initial,
                peak_private = peak,
                final_private = final_b,
                peak_to_final_bytes = peak as i64 - final_b as i64,
                growth = final_b as i64 - initial as i64,
                "showhide.soak.report"
            );
            let _ = slint::quit_event_loop();
        });
    }

    // must NOT use ui.run(): Slint exits the loop when all windows are hidden;
    // the launcher has to stay resident for hotkey + tray.
    slint::run_event_loop_until_quit()?;
    info!("launcher.stopped");
    Ok(())
}

/// Current private (commit) bytes of this process; 0 = unavailable.
#[cfg(windows)]
fn private_bytes() -> u64 {
    use windows::Win32::System::ProcessStatus::{
        GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS, PROCESS_MEMORY_COUNTERS_EX,
    };
    unsafe {
        let mut pmc = PROCESS_MEMORY_COUNTERS_EX {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS_EX>() as u32,
            ..Default::default()
        };
        let ok = GetProcessMemoryInfo(
            windows::Win32::Foundation::HANDLE(-1isize as *mut _),
            &mut pmc as *mut _ as *mut PROCESS_MEMORY_COUNTERS,
            pmc.cb,
        );
        if ok.is_ok() {
            pmc.PrivateUsage as u64
        } else {
            0
        }
    }
}

#[cfg(not(windows))]
fn private_bytes() -> u64 {
    0
}
