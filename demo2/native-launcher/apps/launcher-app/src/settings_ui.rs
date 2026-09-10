//! Native settings surface (P3.0-F04): settings exposed as searchable
//! commands whose actions the host applies — no new UI mode, the existing
//! result list + ActionPanel patterns carry it.
//!
//! Write policy (UX-R4): only the whitelisted fields below are written;
//! the config is saved through `launcher_config::save` (atomic tmp+rename)
//! and hot reload (MUST-2) picks changes up on the next popup open.
//!
//! v1 scope: theme cycle / autostart toggle / index-dir removal / open
//! config. Full-reindex trigger and hotkey capture are DEFERRED (see the
//! B0 batch record).

use launcher_domain::{Action, ActionKind, Category, Command, QueryContext};

pub const PROVIDER_ID: &str = "settings-ui";

fn base_cmd(id: &str, title: &str, subtitle: &str, target: &str) -> Command {
    Command {
        id: id.into(),
        title: title.into(),
        subtitle: Some(subtitle.into()),
        icon: None,
        provider_id: PROVIDER_ID.into(),
        score: 0.0,
        keywords: vec!["settings".into()],
        category: Category::Command,
        actions: vec![Action {
            kind: ActionKind::Execute,
            payload: None,
            id: Some("apply".into()),
            title: Some("Apply".into()),
            disabled_reason: None,
            shortcut: None,
            confirmation_required: false,
        }],
        target: Some(target.into()),
    }
}

/// Build the settings command list for the query (keyword-filtered).
pub fn settings_commands(q: &QueryContext, cfg: &launcher_config::AppConfig) -> Vec<Command> {
    let theme_next = next_theme(&cfg.theme_mode);
    let autostart_label = if cfg.autostart { "Disable" } else { "Enable" };
    let mut cmds = vec![
        base_cmd(
            "settings:theme",
            &format!("Settings · Theme: switch to {theme_next}"),
            &format!("current: {}", cfg.theme_mode),
            "theme",
        ),
        base_cmd(
            "settings:autostart",
            &format!("Settings · Autostart: {autostart_label}"),
            "launch on login",
            "autostart",
        ),
        base_cmd(
            "settings:config",
            "Settings · Open config.toml",
            "edit advanced options (hotkey, index dirs, [llm])",
            "config",
        ),
    ];
    for dir in &cfg.index_dirs {
        cmds.push(base_cmd(
            &format!("settings:idx:{}", launcher_domain::normalize_path_identity(dir)),
            &format!("Settings · Remove index dir: {dir}"),
            "takes effect after restart",
            &format!("index:{dir}"),
        ));
    }
    // keyword filter beyond "settings": match against title words
    let wants = |kw: &str| q.normalized.is_empty() || q.normalized.contains(kw);
    cmds.retain(|c| {
        let t = c.title.to_lowercase();
        if wants("theme") && t.contains("theme") {
            return true;
        }
        if wants("autostart") && t.contains("autostart") {
            return true;
        }
        if wants("config") && t.contains("config") {
            return true;
        }
        if (wants("index") || wants("dir")) && t.contains("index dir") {
            return true;
        }
        // bare "settings" query: everything
        q.normalized.split_whitespace().all(|w| w == "settings") || q.normalized.is_empty()
    });
    cmds
}

fn next_theme(current: &str) -> &'static str {
    match current {
        "light" => "system",
        "system" => "dark",
        _ => "light",
    }
}

/// Apply one settings action (host-routed via the `settings-ui` namespace).
/// Returns a user-facing status line.
pub fn apply(target: &str) -> anyhow::Result<String> {
    let cfg_path = launcher_config::config_path()?;
    let mut cfg = launcher_config::load_or_create(&cfg_path)?;
    match target {
        "theme" => {
            cfg.theme_mode = next_theme(&cfg.theme_mode).into();
            launcher_config::save(&cfg_path, &cfg)?;
            Ok(format!("Theme → {} (next popup open)", cfg.theme_mode))
        }
        "autostart" => {
            cfg.autostart = !cfg.autostart;
            launcher_config::save(&cfg_path, &cfg)?;
            crate::autostart::set_autostart(cfg.autostart)?;
            Ok(format!(
                "Autostart {}",
                if cfg.autostart { "enabled" } else { "disabled" }
            ))
        }
        "config" => {
            // reuse the settings command's editor opening path
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("notepad").arg(&cfg_path).spawn();
            }
            Ok("Opened config.toml".into())
        }
        t if t.starts_with("index:") => {
            let dir = t.strip_prefix("index:").unwrap_or_default();
            cfg.index_dirs.retain(|d| launcher_domain::normalize_path_identity(d) != dir);
            launcher_config::save(&cfg_path, &cfg)?;
            Ok(format!("Index dir removed (takes effect after restart): {dir}"))
        }
        _ => anyhow::bail!("unknown settings target: {target}"),
    }
}

/// The provider: surfaces settings commands for settings-ish queries.
pub struct SettingsUiProvider;

impl launcher_core::Provider for SettingsUiProvider {
    fn id(&self) -> &str {
        PROVIDER_ID
    }

    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        let n = q.normalized.clone();
        let is_settings = n.contains("settings") || n.contains("设置");
        if !is_settings {
            return vec![];
        }
        match launcher_config::load_or_create(&launcher_config::config_path().unwrap_or_default())
        {
            Ok(cfg) => settings_commands(q, &cfg),
            Err(_) => vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg() -> launcher_config::AppConfig {
        let mut c = launcher_config::AppConfig::default();
        c.index_dirs = vec!["C:\\docs".into()];
        c
    }

    /// SET-1 (partial): settings queries surface the command set.
    #[test]
    fn settings_query_surfaces_commands() {
        let q = QueryContext::parse("settings");
        let cmds = settings_commands(&q, &cfg());
        let ids: Vec<&str> = cmds.iter().map(|c| c.id.as_str()).collect();
        assert!(ids.contains(&"settings:theme"));
        assert!(ids.contains(&"settings:autostart"));
        assert!(ids.contains(&"settings:config"));
        assert!(ids.iter().any(|i| i.starts_with("settings:idx:")));
    }

    /// keyword filtering: "settings theme" narrows to the theme command.
    #[test]
    fn keyword_filter_narrows() {
        let q = QueryContext::parse("settings theme");
        let cmds = settings_commands(&q, &cfg());
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].id, "settings:theme");
    }

    /// non-settings queries produce nothing.
    #[test]
    fn non_settings_query_empty() {
        assert!(settings_commands(&QueryContext::parse("chrome"), &cfg()).is_empty());
    }
}
