//! Workflow catalog provider (Launcher 1.0 MUST-1): installed workflow
//! definitions (`<data>/workflows/*.json`) surfaced as searchable Commands.
//!
//! Routing contract: this provider only DESCRIBES workflows. Execution is
//! routed by the host through the `workflows` provider namespace into
//! `workflow_service::start_workflow` (WorkflowRunner → ReferenceResolver →
//! ActionResolver → Engine) — the provider itself never executes anything
//! and the engine never sees a workflow file as an Open target.

use std::path::{Path, PathBuf};

use launcher_domain::{Action, ActionKind, ActionPayload, Category, Command, QueryContext};
use tracing::warn;

use crate::Provider;

pub const PROVIDER_ID: &str = "workflows";

pub struct WorkflowCatalogProvider {
    dir: PathBuf,
    commands: Vec<Command>,
}

impl WorkflowCatalogProvider {
    /// Load every `*.json` in `dir` as an installed workflow. Invalid files
    /// are logged and skipped (one broken definition must not hide the rest).
    pub fn load_dir(dir: &Path) -> Self {
        let mut commands = Vec::new();
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut files: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e.eq_ignore_ascii_case("json")))
                .collect();
            files.sort();
            for file in files {
                match load_definition(&file) {
                    Ok(def) => {
                        let stem = file
                            .file_stem()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_default();
                        commands.push(Command {
                            id: format!("workflow:{stem}"),
                            title: def.name.clone(),
                            subtitle: Some(format!("Workflow · {} steps", def.steps.len())),
                            icon: None,
                            provider_id: PROVIDER_ID.into(),
                            score: 0.0,
                            keywords: vec![def.name.to_lowercase(), stem, "workflow".into()],
                            category: Category::Command,
                            actions: vec![Action {
                                kind: ActionKind::Open,
                                payload: Some(ActionPayload::Path(
                                    file.to_string_lossy().to_string(),
                                )),
                                id: Some("run".into()),
                                title: Some("Run workflow".into()),
                                disabled_reason: None,
                                shortcut: None,
                                confirmation_required: false,
                            }],
                            target: Some(file.to_string_lossy().to_string()),
                        });
                    }
                    Err(e) => {
                        warn!(file = %file.display(), error = %e, "invalid workflow definition skipped");
                    }
                }
            }
        }
        Self {
            dir: dir.to_path_buf(),
            commands,
        }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

/// Parse and validate one workflow definition file.
pub fn load_definition(path: &Path) -> Result<launcher_domain::WorkflowDefinition, String> {
    let raw = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    let def: launcher_domain::WorkflowDefinition =
        serde_json::from_str(&raw).map_err(|e| e.to_string())?;
    def.validate()?;
    Ok(def)
}

impl Provider for WorkflowCatalogProvider {
    fn id(&self) -> &str {
        PROVIDER_ID
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        // installed workflows are a small bounded set: return all and let the
        // global ranker filter (empty query = full catalog for discovery)
        let _ = q;
        self.commands.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_workflow(dir: &Path, name: &str, id: &str, title: &str) -> PathBuf {
        std::fs::create_dir_all(dir).unwrap();
        let p = dir.join(name);
        std::fs::write(
            &p,
            serde_json::json!({
                "id": id, "version": 1, "name": title,
                "steps": [ { "step_id": "s1", "action": { "Inline": {
                    "id": "s1", "title": "Copy", "type": "system.copy_to_clipboard",
                    "input": {"text": "hi"} } }, "input": {"text": "hi"} } ]
            })
            .to_string(),
        )
        .unwrap();
        p
    }

    #[test]
    fn loads_valid_definitions_as_commands() {
        let dir = std::env::temp_dir().join("nl_wf_ok");
        let _ = std::fs::remove_dir_all(&dir);
        write_workflow(&dir, "demo.json", "wf.demo", "Demo Workflow");
        let mut provider = WorkflowCatalogProvider::load_dir(&dir);
        assert_eq!(provider.len(), 1);
        let cmds = provider.query(&QueryContext::parse(""));
        assert_eq!(cmds[0].provider_id, "workflows");
        assert_eq!(cmds[0].title, "Demo Workflow");
        assert_eq!(cmds[0].actions[0].id.as_deref(), Some("run"));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn invalid_definitions_are_skipped_not_fatal() {
        let dir = std::env::temp_dir().join("nl_wf_bad");
        let _ = std::fs::remove_dir_all(&dir);
        write_workflow(&dir, "good.json", "wf.good", "Good");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("broken.json"), "{ not json").unwrap();
        std::fs::write(dir.join("empty-steps.json"), r#"{"id":"x","name":"X","steps":[]}"#)
            .unwrap();
        let provider = WorkflowCatalogProvider::load_dir(&dir);
        assert_eq!(provider.len(), 1, "only the valid definition loads");
        std::fs::remove_dir_all(&dir).ok();
    }
}
