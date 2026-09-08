//! Recent files provider: reads `%APPDATA%\Microsoft\Windows\Recent` `.lnk`
//! shortcuts, ported from demo1. Unit tests inject a temp directory.

use std::path::{Path, PathBuf};

use launcher_core::Provider;
use launcher_domain::{Action, ActionKind, ActionPayload, Category, Command, QueryContext};
use tracing::{debug, info};

/// Bounded catalog (spec 4.2).
pub const MAX_RECENT: usize = 500;

pub struct RecentFilesProvider {
    recent_dir: PathBuf,
    entries: Vec<RecentEntry>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecentEntry {
    /// Display name (`.lnk` file stem, usually the original file name).
    pub name: String,
    /// Resolved target path; falls back to the `.lnk` path itself.
    pub target: PathBuf,
    /// `.lnk` modification time (ms since epoch) — the recency signal.
    pub recency_ms: u64,
}

impl RecentFilesProvider {
    pub fn new() -> Self {
        let dir = std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_default()
            .join(r"Microsoft\Windows\Recent");
        Self {
            recent_dir: dir,
            entries: Vec::new(),
        }
    }

    /// Constructor with injected directory (unit tests).
    pub fn with_dir(dir: PathBuf) -> Self {
        Self {
            recent_dir: dir,
            entries: Vec::new(),
        }
    }

    /// Scan `.lnk` files and resolve targets. Individual failures are logged
    /// and skipped.
    pub fn scan(&self) -> anyhow::Result<Vec<RecentEntry>> {
        let rd = std::fs::read_dir(&self.recent_dir)
            .map_err(|e| anyhow::anyhow!("read recent dir {}: {e}", self.recent_dir.display()))?;
        let mut out = Vec::new();
        for entry in rd.flatten() {
            let path = entry.path();
            if !path
                .extension()
                .is_some_and(|e| e.eq_ignore_ascii_case("lnk"))
            {
                continue;
            }
            let Some(name) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            let target = resolve_lnk_target(&path).unwrap_or_else(|| path.clone());
            debug!(file = name, target = %target.display(), "recent entry");
            // The .lnk mtime is the OS-maintained recency signal — sorting
            // by it makes "Recent" actually recent (review 64 §17).
            let recency_ms = path
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            out.push(RecentEntry {
                name: name.to_string(),
                target,
                recency_ms,
            });
            if out.len() >= MAX_RECENT {
                break;
            }
        }
        out.sort_by(|a, b| b.recency_ms.cmp(&a.recency_ms));
        Ok(out)
    }

    /// Build the in-memory catalog; call once at startup.
    pub fn build_cache(&mut self) -> anyhow::Result<()> {
        self.entries = self.scan()?;
        info!(count = self.entries.len(), dir = %self.recent_dir.display(), "recent files cache built");
        Ok(())
    }

    pub fn entries(&self) -> &[RecentEntry] {
        &self.entries
    }

    fn command_for(&self, e: &RecentEntry) -> Command {
        Command {
            id: format!("recent:{}", e.target.display()),
            title: e.name.clone(),
            subtitle: Some(e.target.display().to_string()),
            icon: None,
            provider_id: "recent-files".into(),
            score: 0.0,
            keywords: vec![e.name.to_lowercase()],
            category: Category::File,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: Some(ActionPayload::Path(e.target.display().to_string())),

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: Some(e.target.display().to_string()),
        }
    }

    pub fn to_commands(&self) -> Vec<Command> {
        self.entries.iter().map(|e| self.command_for(e)).collect()
    }
}

impl Default for RecentFilesProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl Provider for RecentFilesProvider {
    fn id(&self) -> &str {
        "recent-files"
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if q.normalized.is_empty() {
            return Vec::new();
        }
        self.to_commands()
    }
}

/// Resolve a `.lnk` target with the `lnk` crate; `None` on parse failure.
pub(crate) fn resolve_lnk_target(path: &Path) -> Option<PathBuf> {
    let shell_link = lnk::ShellLink::open(path, lnk::encoding::UTF_16LE).ok()?;
    shell_link.link_target().map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal temp-dir helper (no extra dependency).
    mod tempdir {
        use std::fs;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU32, Ordering};

        static COUNTER: AtomicU32 = AtomicU32::new(0);

        pub struct TempDir(PathBuf);

        impl TempDir {
            pub fn new() -> std::io::Result<Self> {
                let n = COUNTER.fetch_add(1, Ordering::Relaxed);
                let p = std::env::temp_dir().join(format!(
                    "launcher2-recent-test-{}-{}",
                    std::process::id(),
                    n
                ));
                fs::create_dir_all(&p)?;
                Ok(Self(p))
            }

            pub fn path(&self) -> &std::path::Path {
                &self.0
            }
        }

        impl Drop for TempDir {
            fn drop(&mut self) {
                let _ = fs::remove_dir_all(&self.0);
            }
        }
    }

    /// Fabricate Recent-like content: real target files + `.lnk` per target.
    fn setup_fake_recent(tag: &str) -> (tempdir::TempDir, Vec<String>) {
        let dir = tempdir::TempDir::new().unwrap();
        let names: Vec<String> = vec!["报告2026.docx", "budget.xlsx", "notes.txt"]
            .into_iter()
            .map(String::from)
            .collect();
        let docs_dir = dir.path().join("docs").join(tag);
        std::fs::create_dir_all(&docs_dir).unwrap();
        for n in &names {
            let target = docs_dir.join(n);
            std::fs::write(&target, b"test").unwrap();
            let link_path = dir.path().join(format!("{n}.lnk"));
            let link = lnk::ShellLink::new_simple(&target).unwrap();
            link.save(&link_path).unwrap();
        }
        (dir, names)
    }

    #[test]
    fn scan_parses_fake_lnk_files() {
        let (dir, names) = setup_fake_recent("scan");
        let provider = RecentFilesProvider::with_dir(dir.path().to_path_buf());
        let entries = provider
            .scan()
            .expect("scan on valid temp dir should not fail");
        assert_eq!(entries.len(), names.len());
        for e in &entries {
            // lnk 0.6 new_simple does not write LINK_INFO; link_target() is
            // None -> documented fallback to the .lnk path itself.
            let expected = dir.path().join(format!("{}.lnk", e.name));
            assert_eq!(
                e.target, expected,
                "fallback target should be the .lnk path"
            );
        }
    }

    #[test]
    fn missing_dir_returns_err() {
        let provider = RecentFilesProvider::with_dir(PathBuf::from(r"Z:\definitely\not\here"));
        assert!(provider.scan().is_err());
    }

    #[test]
    fn empty_query_returns_empty() {
        let mut p = RecentFilesProvider::with_dir(PathBuf::from(r"Z:\nowhere"));
        assert!(p.query(&QueryContext::parse("")).is_empty());
        let _ = p.build_cache();
    }

    #[test]
    fn write_read_query_smoke() {
        let (dir, _) = setup_fake_recent("smoke");
        let mut provider = RecentFilesProvider::with_dir(dir.path().to_path_buf());
        provider.build_cache().unwrap();
        let cmds = provider.query(&QueryContext::parse("docx"));
        assert!(cmds.iter().any(|c| c.title == "报告2026.docx"));
        // every command carries an Open action on the resolved target
        assert!(cmds
            .iter()
            .all(|c| !c.actions.is_empty() && c.target.is_some()));
    }
}
