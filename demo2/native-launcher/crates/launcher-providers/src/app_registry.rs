//! Application enumeration: Start Menu `.lnk` + registry Uninstall keys
//! (HKCU/HKLM/WOW6432Node), ported from demo1's AppEnumProvider.
//!
//! Enumerated once at startup into a bounded in-memory catalog; no FS watch
//! (new installs appear after restart — known MVP limitation).

use std::path::{Path, PathBuf};

use crate::recent_files::resolve_lnk_target;
use launcher_core::Provider;
use launcher_domain::{Action, ActionKind, ActionPayload, Category, Command, QueryContext};
use tracing::{debug, info, warn};

/// Hard bound protecting memory (spec 4.2).
pub const MAX_APPS: usize = 5000;

/// A raw application entry produced by enumeration.
#[derive(Debug, Clone, PartialEq)]
pub struct AppEntry {
    pub name: String,
    pub path: PathBuf,
    /// Resolved shortcut target (non-shortcut entries: same as `path` or
    /// the registry-derived exe). `None` = resolution failed/absent.
    pub resolved_target: Option<PathBuf>,
    /// Discovery sources merged INTO this record by identity resolution
    /// (review 72 §5); `source` remains the canonical/primary one.
    pub merged_sources: Vec<&'static str>,
    pub source: &'static str,
}

/// Provider that answers queries from an enumerated app catalog.
pub struct AppRegistryProvider {
    entries: Vec<AppEntry>,
}

/// P2-C actions for an application entry: Open (primary), Run as
/// administrator (only for executables) and Copy path. Secondary actions
/// carry stable ids so the host can execute them by (command_id, action_id).
fn app_actions(path: &str, resolved_target: Option<&Path>) -> Vec<Action> {
    let mut actions = vec![Action {
        kind: ActionKind::Open,
        payload: Some(ActionPayload::Path(path.to_string())),
        id: None,
        title: None,
        disabled_reason: None,
        shortcut: None,
        confirmation_required: false,
    }];
    // Run-as-administrator targets the RESOLVED executable — a Start Menu
    // entry is a .lnk, so the check is on the target, not the entry path.
    let admin_target = resolved_target
        .map(|p| p.to_string_lossy().to_string())
        .filter(|t| t.to_lowercase().ends_with(".exe"))
        .or_else(|| {
            (path.to_lowercase().ends_with(".exe")).then(|| path.to_string())
        });
    if let Some(exe) = admin_target {
        actions.push(Action {
            kind: ActionKind::RunAsAdmin,
            payload: Some(ActionPayload::Path(exe)),
            id: Some("runas".into()),
            title: Some("Run as administrator".into()),
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        });
    }
    actions.push(Action {
        kind: ActionKind::Copy,
        payload: Some(ActionPayload::Path(path.to_string())),
        id: Some("copypath".into()),
        title: Some("Copy path".into()),
        disabled_reason: None,
        shortcut: None,
        confirmation_required: false,
    });
    actions
}

impl AppRegistryProvider {
    pub fn from_entries(entries: Vec<AppEntry>) -> Self {
        // P2.1 §21/22 (INV-IDENTITY-001): equivalent discovery records for
        // the same application collapse to ONE entry — exact duplicate first,
        // then executable-identity merge (Start Menu Chrome.lnk and the
        // registry's chrome.exe are the same app to the user).
        let mut entries = merge_by_executable(dedup(entries));
        entries.truncate(MAX_APPS);
        Self { entries }
    }

    /// P2.4-A05: authoritative catalog read path — the provider is built FROM
    /// the persisted catalog (already canonical-identity merged at reconcile
    /// time), never from a synchronous re-discovery on the search path.
    /// Stable-id rule: `path` keeps the ORIGINAL discovery entry path
    /// (`CatalogRecord::entry_path`, e.g. the .lnk) so command_ids and the
    /// history/favorites join are unchanged. Non-ready records (stale/broken)
    /// are excluded — catalog lifecycle is metadata and must never become a
    /// candidate on its own (P2.4-A04: metadata != authority).
    pub fn from_catalog_records(records: &[crate::catalog::CatalogRecord]) -> Self {
        let entries = records
            .iter()
            .filter(|r| r.lifecycle == crate::catalog::CatalogLifecycle::Ready)
            .map(|r| {
                let target = PathBuf::from(&r.launch_path);
                let resolved = is_exe_path(&target).then(|| target.clone());
                AppEntry {
                    name: r.display_name.clone(),
                    path: PathBuf::from(&r.entry_path),
                    resolved_target: resolved,
                    merged_sources: r.sources.iter().map(|s| source_static(s)).collect(),
                    source: source_static(&r.source),
                }
            })
            .collect();
        Self::from_entries(entries)
    }

    /// Enumerate all sources. Independent sources: one failing source only
    /// logs a WARN.
    pub fn collect_entries() -> Vec<AppEntry> {
        let t0 = std::time::Instant::now();
        let mut entries = Vec::new();
        for dir in start_menu_dirs() {
            collect_lnk_dir(&dir, &mut entries);
        }
        match collect_uninstall_entries() {
            Ok(mut reg) => entries.append(&mut reg),
            Err(e) => warn!(error = %e, "failed to read Uninstall registry keys"),
        }
        let out = dedup(entries);
        info!(
            elapsed_ms = t0.elapsed().as_millis() as u64,
            total = out.len(),
            "app enumeration done"
        );
        out
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn entries(&self) -> &[AppEntry] {
        &self.entries
    }

    fn command_for(&self, e: &AppEntry) -> Command {
        let path_str = e.path.display().to_string();
        // `target` carries the canonical executable (P2.1-C semantic
        // identity); the Open action still opens the shortcut itself.
        let identity_target = e
            .resolved_target
            .as_deref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| path_str.clone());
        Command {
            id: format!("appreg:{path_str}"),
            title: e.name.clone(),
            subtitle: Some(path_str.clone()),
            icon: None,
            provider_id: "app-registry".into(),
            score: 0.0,
            keywords: vec![e.name.to_lowercase()],
            category: Category::Application,
            actions: app_actions(&path_str, e.resolved_target.as_deref()),
            target: Some(identity_target),
        }
    }

    pub fn to_commands(&self) -> Vec<Command> {
        self.entries.iter().map(|e| self.command_for(e)).collect()
    }
}

impl Provider for AppRegistryProvider {
    fn id(&self) -> &str {
        "app-registry"
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if q.normalized.is_empty() {
            return Vec::new();
        }
        self.to_commands()
    }
}

fn start_menu_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(appdata) = std::env::var_os("APPDATA").map(PathBuf::from) {
        dirs.push(appdata.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    if let Some(common) = std::env::var_os("PROGRAMDATA").map(PathBuf::from) {
        dirs.push(common.join(r"Microsoft\Windows\Start Menu\Programs"));
    }
    dirs.retain(|p| p.exists());
    dirs
}

fn collect_lnk_dir(dir: &Path, out: &mut Vec<AppEntry>) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        debug!(dir = %dir.display(), "start menu dir missing, skipped");
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_lnk_dir(&path, out);
        } else if path
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
        {
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            // Resolve the .lnk target (review 64 §32-10): the resolved exe
            // enables Run-as-administrator and improves app identity; the
            // .lnk itself stays the Open target (ShellExecute resolves it).
            let resolved = resolve_lnk_target(&path).unwrap_or_else(|| path.clone());
            out.push(AppEntry {
                name: name.to_string(),
                path,
                resolved_target: Some(resolved),
                merged_sources: Vec::new(),
                source: "start-menu",
            });
        }
    }
}

/// Read Uninstall keys (HKCU + HKLM + WOW6432Node); keep entries with a
/// non-empty DisplayName and an .exe target.
#[cfg(windows)]
fn collect_uninstall_entries() -> anyhow::Result<Vec<AppEntry>> {
    use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
    use winreg::RegKey;

    let keys = [
        (
            HKEY_CURRENT_USER,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
        (
            HKEY_LOCAL_MACHINE,
            r"Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall",
        ),
    ];
    let mut out = Vec::new();
    for (hive, path) in keys {
        let Ok(root) = RegKey::predef(hive).open_subkey(path) else {
            continue;
        };
        for sub in root.enum_keys().flatten() {
            let Ok(key) = root.open_subkey(&sub) else {
                continue;
            };
            let Some(name) = key
                .get_value::<String, _>("DisplayName")
                .ok()
                .filter(|n| !n.trim().is_empty())
            else {
                continue;
            };
            if key
                .get_value::<u32, _>("SystemComponent")
                .map(|v| v != 0)
                .unwrap_or(false)
            {
                continue;
            }
            let exe = key
                .get_value::<String, _>("DisplayIcon")
                .ok()
                .and_then(display_icon_to_exe)
                .or_else(|| {
                    key.get_value::<String, _>("InstallLocation")
                        .ok()
                        .map(PathBuf::from)
                });
            let Some(exe) =
                exe.filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")))
            else {
                continue;
            };
            out.push(AppEntry {
                name,
                resolved_target: Some(exe.clone()),
                merged_sources: Vec::new(),
                path: exe,
                source: "uninstall",
            });
        }
    }
    Ok(out)
}

#[cfg(not(windows))]
fn collect_uninstall_entries() -> anyhow::Result<Vec<AppEntry>> {
    Ok(Vec::new())
}

/// `DisplayIcon` is commonly `C:\x\app.exe` or `C:\x\app.exe,0`; take the exe.
fn display_icon_to_exe(raw: String) -> Option<PathBuf> {
    let raw = raw.split(',').next()?.trim().to_string();
    let p = PathBuf::from(&raw);
    (p.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe"))).then_some(p)
}

/// Dedup by (name, path), keeping first occurrence (start menu wins).
/// ApplicationIdentityResolver seam (review 72 §33/§48): v1 = executable
/// identity. Future resolvers (AUMID / MSIX package / portable) implement
/// the same signature and merge into the same ApplicationIdentity space —
/// Search never learns which resolver produced an id.
///
/// INV-IDENTITY-003 (review 74 §11): identity resolution is PURE — it reads
/// metadata / parses / normalizes and MUST NOT produce or execute Effects
/// (no launch, no shell, no network, no install).
fn is_exe_path(p: &Path) -> bool {
    p.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("exe"))
}

/// Map a catalog source id to the provider's 'static source vocabulary.
/// Unknown ids degrade to "catalog" (never panic, never lose the row).
fn source_static(s: &str) -> &'static str {
    match s {
        "start-menu" => "start-menu",
        "uninstall" => "uninstall",
        "packaged" => "packaged",
        "app-paths" => "app-paths",
        "portable" => "portable",
        _ => "catalog",
    }
}

fn executable_identity(resolved_target: Option<&std::path::Path>) -> Option<String> {
    let t = resolved_target?.to_string_lossy().to_string();
    t.to_lowercase().ends_with(".exe").then(|| {
        launcher_domain::normalize_path_identity(&t)
    })
}

/// P2.1-D Portable discovery (review 78 §29-31): ONLY configured roots,
/// bounded depth, bounded count — never a whole-disk scan. Every exe found
/// becomes a candidate; the exe-identity merge deduplicates it against
/// Start Menu/Registry records of the same application.
pub fn portable_entries(roots: &[PathBuf], max: usize, max_depth: u32) -> Vec<AppEntry> {
    let mut out = Vec::new();
    for root in roots {
        if !root.exists() {
            continue;
        }
        scan_portable(root, 0, max_depth, max, &mut out);
        if out.len() >= max {
            break;
        }
    }
    out
}

fn scan_portable(dir: &Path, depth: u32, max_depth: u32, max: usize, out: &mut Vec<AppEntry>) {
    if depth > max_depth || out.len() >= max {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan_portable(&path, depth + 1, max_depth, max, out);
        } else if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("exe")) {
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else { continue };
            // deterministic display cleanup only (review 78 §63)
            let mut chars = name.chars();
            let display = match chars.next() {
                Some(f) => f.to_uppercase().collect::<String>() + chars.as_str(),
                None => continue,
            };
            if out.len() >= max {
                return;
            }
            out.push(AppEntry {
                name: display,
                resolved_target: Some(path.clone()),
                path: path.clone(),
                merged_sources: Vec::new(),
                source: "portable",
            });
        }
    }
}

/// Second-pass identity merge: group by the canonical resolved executable.
/// Source priority: start-menu (best display name) > uninstall. Only entries
/// whose resolved target IS an executable participate; entries without a
/// resolution keep their own identity. Discovery sources are PRESERVED on
/// the kept entry (review 72 §5) so future staleness checks can tell "the
/// registry source vanished" from "the app is gone".
fn merge_by_executable(entries: Vec<AppEntry>) -> Vec<AppEntry> {
    use std::collections::HashMap;
    let mut by_exe: HashMap<String, usize> = HashMap::new();
    let mut out: Vec<AppEntry> = Vec::new();
    for mut e in entries {
        match executable_identity(e.resolved_target.as_deref()) {
            Some(id) => match by_exe.get(&id) {
                Some(&keep) if out[keep].source == "start-menu" => {
                    out[keep].merged_sources.push(e.source);
                }
                Some(&keep) => {
                    // a start-menu record replaces a weaker duplicate and
                    // absorbs its discovery sources
                    if e.source == "start-menu" {
                        e.merged_sources.push(out[keep].source);
                        out[keep] = e;
                    } else {
                        out[keep].merged_sources.push(e.source);
                    }
                }
                None => {
                    by_exe.insert(id, out.len());
                    out.push(e);
                }
            },
            None => out.push(e),
        }
    }
    out
}

fn dedup(entries: Vec<AppEntry>) -> Vec<AppEntry> {
    let mut seen = std::collections::HashSet::new();
    entries
        .into_iter()
        .filter(|e| {
            seen.insert((
                e.name.to_lowercase(),
                e.path.to_string_lossy().to_lowercase(),
            ))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{CatalogLifecycle, CatalogRecord};

    fn rec(identity: &str, name: &str, source: &str, launch: &str, entry: &str,
           lifecycle: CatalogLifecycle, sources: Vec<String>) -> CatalogRecord {
        CatalogRecord {
            identity_key: identity.into(),
            display_name: name.into(),
            source: source.into(),
            launch_path: launch.into(),
            lifecycle,
            sources,
            entry_path: entry.into(),
        }
    }

    /// P2.4-A05 acceptance: the provider is built FROM catalog records;
    /// same-identity rows are already merged by reconcile; non-ready
    /// lifecycle is excluded; entry_path keeps command_ids stable.
    #[test]
    fn from_catalog_records_merges_excludes_non_ready_and_preserves_ids() {
        let records = vec![
            rec(r"win32:c:\tools\app.exe", "App", "start-menu",
                r"C:\Tools\App.exe", r"C:\SM\App.lnk",
                CatalogLifecycle::Ready, vec!["start-menu".into(), "uninstall".into()]),
            rec(r"win32:c:\tools\stale.exe", "Stale App", "start-menu",
                r"C:\Tools\stale.exe", r"C:\SM\stale.lnk",
                CatalogLifecycle::Stale, vec!["start-menu".into()]),
            rec(r"win32:c:\tools\broken.exe", "Broken App", "start-menu",
                r"C:\Tools\broken.exe", r"C:\SM\broken.lnk",
                CatalogLifecycle::Broken, vec!["start-menu".into()]),
        ];
        let p = AppRegistryProvider::from_catalog_records(&records);
        let cmds = p.to_commands();
        assert_eq!(cmds.len(), 1, "stale/broken records never become candidates");
        let c = &cmds[0];
        // stable id: the ORIGINAL entry path (the .lnk), not the resolved target
        assert_eq!(c.id, r"appreg:C:\SM\App.lnk");
        // identity target still carries the resolved exe (search identity)
        assert_eq!(c.target.as_deref(), Some(r"C:\Tools\App.exe"));
    }

    #[test]
    fn display_icon_parsing() {
        assert_eq!(
            display_icon_to_exe(r"C:\x\app.exe,0".to_string()),
            Some(PathBuf::from(r"C:\x\app.exe"))
        );
        assert_eq!(display_icon_to_exe(r"C:\x\icon.ico".to_string()), None);
    }

    /// P2.1 §76 identity E2E (unit level): the same application discovered
    /// via Start Menu shortcut AND registry must collapse to one record.
    #[test]
    fn same_application_from_two_sources_merges_to_one() {
        let entries = vec![
            AppEntry {
                name: "Google Chrome".into(),
                merged_sources: Vec::new(),
                resolved_target: Some(PathBuf::from(
                    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
                )),
                path: PathBuf::from(
                    r"C:\ProgramData\Microsoft\Windows\Start Menu\Chrome.lnk",
                ),
                source: "start-menu",
            },
            AppEntry {
                name: "Chrome".to_string(),
                merged_sources: Vec::new(),
                resolved_target: Some(PathBuf::from(
                    r"C:\Program Files\Google\Chrome\Application\CHROME.EXE",
                )),
                path: PathBuf::from(r"C:\Program Files\Google\Chrome\Application\chrome.exe"),
                source: "uninstall",
            },
        ];
        let provider = AppRegistryProvider::from_entries(entries);
        assert_eq!(provider.len(), 1, "one application, two discovery sources");
        assert_eq!(provider.entries()[0].name, "Google Chrome");
        assert_eq!(provider.entries()[0].source, "start-menu");
    }

    /// review 72 §5: merged records keep their discovery sources, so a
    /// future staleness check can see the registry source vanish.
    #[test]
    fn merge_preserves_discovery_sources() {
        let entries = vec![
            AppEntry {
                name: "Google Chrome".into(),
                merged_sources: Vec::new(),
                resolved_target: Some(PathBuf::from(r"C:\x\chrome.exe")),
                path: PathBuf::from(r"C:\lnks\Chrome.lnk"),
                source: "start-menu",
            },
            AppEntry {
                name: "Chrome".to_string(),
                merged_sources: Vec::new(),
                resolved_target: Some(PathBuf::from(r"C:\X\chrome.exe")),
                path: PathBuf::from(r"C:\x\chrome.exe"),
                source: "uninstall",
            },
        ];
        let provider = AppRegistryProvider::from_entries(entries);
        assert_eq!(provider.entries()[0].source, "start-menu");
        assert_eq!(provider.entries()[0].merged_sources, vec!["uninstall"]);
    }

    #[test]
    fn distinct_executables_do_not_merge() {
        let entries = vec![
            AppEntry {
                name: "Chrome".to_string(),
                merged_sources: Vec::new(),
                resolved_target: Some(PathBuf::from(r"C:\chrome.exe")),
                path: PathBuf::from(r"C:\chrome.exe"),
                source: "uninstall",
            },
            AppEntry {
                name: "Chromium".to_string(),
                merged_sources: Vec::new(),
                resolved_target: Some(PathBuf::from(r"C:\chromium.exe")),
                path: PathBuf::from(r"C:\chromium.exe"),
                source: "uninstall",
            },
        ];
        let provider = AppRegistryProvider::from_entries(entries);
        assert_eq!(provider.len(), 2);
    }

    #[test]
    fn dedup_keeps_first() {
        let out = dedup(vec![
            AppEntry {
                name: "App".into(),
                resolved_target: None,
                merged_sources: Vec::new(),
                path: PathBuf::from(r"C:\a.exe"),
                source: "start-menu",
            },
            AppEntry {
                name: "app".into(),
                resolved_target: None,
                merged_sources: Vec::new(),
                path: PathBuf::from(r"C:\A.EXE"),
                source: "uninstall",
            },
            AppEntry {
                name: "Other".into(),
                resolved_target: None,
                merged_sources: Vec::new(),
                path: PathBuf::from(r"C:\b.exe"),
                source: "uninstall",
            },
        ]);
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].source, "start-menu");
    }

    #[test]
    fn start_menu_enumeration_finds_something() {
        let dirs = start_menu_dirs();
        let mut entries = Vec::new();
        for d in &dirs {
            collect_lnk_dir(d, &mut entries);
        }
        // real machine: start menu always has some shortcuts
        assert!(!entries.is_empty());
    }

    #[test]
    fn commands_carry_open_action() {
        let p = AppRegistryProvider::from_entries(vec![AppEntry {
            name: "Notepad++".into(),
            resolved_target: None,
            merged_sources: Vec::new(),
            path: PathBuf::from(r"C:\tools\notepad++.exe"),
            source: "uninstall",
        }]);
        let cmds = p.to_commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].title, "Notepad++");
        assert!(matches!(cmds[0].actions[0].kind, ActionKind::Open));
        assert_eq!(cmds[0].target.as_deref(), Some(r"C:\tools\notepad++.exe"));
    }
}

#[cfg(test)]
mod portable_tests {
    use super::*;

    /// P2.1-D Gate D6: portable discovery is bounded — only configured
    /// roots, only up to max_depth, only up to max entries.
    #[test]
    fn portable_scan_bounded_and_correct() {
        let root = std::env::temp_dir().join("nl_portable_test");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("toolA")).unwrap();
        std::fs::write(root.join("toolA").join("toolA.exe"), "x").unwrap();
        // beyond max_depth (2): must NOT be discovered
        std::fs::create_dir_all(root.join("deep").join("d2").join("d3")).unwrap();
        std::fs::write(root.join("deep").join("d2").join("d3").join("deep.exe"), "x").unwrap();

        let entries = portable_entries(&[root.clone()], 64, 2);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "ToolA");
        assert_eq!(entries[0].source, "portable");
        assert!(entries[0]
            .resolved_target
            .as_ref()
            .unwrap()
            .ends_with("toolA.exe"));
        std::fs::remove_dir_all(&root).ok();
    }

    /// review 78 §41: a portable exe and a Start Menu shortcut resolving to
    /// the same executable merge into ONE application entry.
    #[test]
    fn portable_and_startmenu_same_exe_merge() {
        let exe = PathBuf::from(r"C:\portable\vivaldi.exe");
        let entries = vec![
            AppEntry {
                name: "Vivaldi".into(),
                resolved_target: Some(exe.clone()),
                path: exe.clone(),
                merged_sources: Vec::new(),
                source: "portable",
            },
            AppEntry {
                name: "Vivaldi Browser".into(),
                resolved_target: Some(PathBuf::from(r"C:\PORTABLE\VIVALDI.EXE")),
                path: PathBuf::from(r"C:\lnks\Vivaldi.lnk"),
                merged_sources: Vec::new(),
                source: "start-menu",
            },
        ];
        let provider = AppRegistryProvider::from_entries(entries);
        assert_eq!(provider.len(), 1, "portable + start-menu same exe merge");
        assert_eq!(provider.entries()[0].name, "Vivaldi Browser");
        assert_eq!(provider.entries()[0].source, "start-menu");
        assert_eq!(provider.entries()[0].merged_sources, vec!["portable"]);
    }
}
