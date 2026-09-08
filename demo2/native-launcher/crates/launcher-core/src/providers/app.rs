//! Application Provider: scans Start-Menu-like directories once at startup
//! into a bounded in-memory catalog, then answers queries from it.

use std::path::PathBuf;

use launcher_domain::{Action, ActionKind, ActionPayload, Category, Command, QueryContext};
use tracing::warn;

use crate::Provider;

/// Hard bound on catalog size to protect memory (spec 4.2).
pub const MAX_APPS: usize = 2000;

pub struct AppProvider {
    apps: Vec<CatalogEntry>,
}

struct CatalogEntry {
    id: String,
    title: String,
    path: String,
}

impl AppProvider {
    pub fn from_dirs(dirs: &[PathBuf]) -> Self {
        let mut apps = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for dir in dirs {
            scan(dir, 0, &mut apps, &mut seen);
        }
        apps.truncate(MAX_APPS);
        Self { apps }
    }

    pub fn len(&self) -> usize {
        self.apps.len()
    }

    pub fn is_empty(&self) -> bool {
        self.apps.is_empty()
    }

    pub fn to_commands(&self) -> Vec<Command> {
        self.apps.iter().map(|e| self.command_for(e)).collect()
    }

    fn command_for(&self, e: &CatalogEntry) -> Command {
        Command {
            id: e.id.clone(),
            title: e.title.clone(),
            subtitle: Some(e.path.clone()),
            icon: None,
            provider_id: "apps".into(),
            score: 0.0,
            keywords: vec![e.title.to_lowercase()],
            category: Category::Application,
            actions: vec![Action {
                kind: ActionKind::Open,
                payload: Some(ActionPayload::Path(e.path.clone())),

                id: None,
                title: None,
                disabled_reason: None,
                shortcut: None,
                confirmation_required: false,
            }],
            target: Some(e.path.clone()),
        }
    }
}

fn scan(
    dir: &std::path::Path,
    depth: u32,
    out: &mut Vec<CatalogEntry>,
    seen: &mut std::collections::HashSet<String>,
) {
    if depth > 6 || out.len() >= MAX_APPS {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            scan(&path, depth + 1, out, seen);
        } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
            let ext = ext.to_lowercase();
            if ext == "lnk" || ext == "exe" {
                let title = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                let path_str = path.to_string_lossy().to_string();
                if seen.insert(path_str.clone()) {
                    out.push(CatalogEntry {
                        id: path_str.clone(),
                        title,
                        path: path_str,
                    });
                }
                if out.len() >= MAX_APPS {
                    warn!("app catalog bound reached");
                    return;
                }
            }
        }
    }
}

impl Provider for AppProvider {
    fn id(&self) -> &str {
        "apps"
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        if q.normalized.is_empty() {
            return Vec::new();
        }
        self.apps.iter().map(|e| self.command_for(e)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_app(dir: &std::path::Path, name: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(dir.join(name), b"MZ").unwrap();
    }

    #[test]
    fn scans_directories_and_dedups() {
        let tmp = std::env::temp_dir().join(format!("app-test-{}", std::process::id()));
        let d1 = tmp.join("a");
        let d2 = tmp.join("b");
        make_app(&d1, "TestApp1.lnk");
        make_app(&d1, "TestTool.exe");
        make_app(&d2, "TestApp2.lnk");
        make_app(&d2, "readme.md");

        let p = AppProvider::from_dirs(&[d1.clone(), d1.clone(), d2.clone()]);
        assert_eq!(p.len(), 3);

        let cmds = p.to_commands();
        let titles: Vec<&str> = cmds.iter().map(|c| c.title.as_str()).collect();
        assert!(titles.contains(&"TestApp1"));
        assert!(titles.contains(&"TestTool"));
        assert!(titles.contains(&"TestApp2"));

        let mut prov = p;
        let q = QueryContext::parse("testapp");
        let hits = prov.query(&q);
        assert_eq!(hits.len(), 3); // provider returns the catalog; ranking happens in core
        assert_eq!(prov.id(), "apps");

        std::fs::remove_dir_all(&tmp).ok();
    }

    #[test]
    fn empty_catalog_ok() {
        let p = AppProvider::from_dirs(&[std::env::temp_dir().join("no-such-dir-xyz")]);
        assert!(p.is_empty());
    }
}
