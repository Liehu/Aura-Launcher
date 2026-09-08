//! TOML configuration (`%APPDATA%\NativeLauncher\config.toml`), ported from demo1.
//!
//! Fault tolerance: missing file -> write defaults; missing fields -> serde
//! defaults; unparsable file -> fall back to defaults with a WARN, never
//! crash the launcher.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tracing::warn;

pub mod handoff;

/// One configured MCP server (MVP4.3 / ADR-0018 / Spec section 21). Only
/// stdio transport in v0.1; credentials/secrets are forbidden in config and
/// belong to a future security/auth milestone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct McpServerConfig {
    /// Stable config-owned identity (Spec section 28).
    pub id: String,
    /// Only `"stdio"` is supported in v0.1.
    #[serde(default = "default_mcp_transport")]
    pub transport: String,
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    /// Wire protocol profile (Phase 11): `"2025-06-18"` (default) or
    /// `"2026-07-28"`. Unknown revisions fail closed at load.
    #[serde(default)]
    pub profile: String,
    /// P0-B: endpoint URL for `transport = "streamable-http"` servers.
    #[serde(default)]
    pub url: Option<String>,
    /// P1-A runtime persistence: `"ephemeral"` (default) or
    /// `"persistent"` (explicit opt-in process reuse, review 56 SS23).
    #[serde(default)]
    pub runtime: String,
    /// P0-B explicit opt-ins (fail-closed defaults, review 48-DD SS6/SS20):
    /// private-network endpoints and non-loopback plaintext HTTP are
    /// rejected unless explicitly allowed here. Credentials stay in the
    /// OS credential store (P0-C) — NEVER in this file.
    #[serde(default)]
    pub allow_private_network: bool,
    #[serde(default)]
    pub allow_plain_http: bool,
}

fn default_mcp_transport() -> String {
    "stdio".into()
}

/// Application configuration.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppConfig {
    /// Global hotkey string, e.g. `"Alt+Space"` or `"Ctrl+Space"`.
    #[serde(default = "default_hotkey")]
    pub hotkey: String,
    /// Accent color, `#RRGGBB`.
    #[serde(default = "default_theme_color")]
    pub theme_color: String,
    /// Launch on login.
    #[serde(default)]
    pub autostart: bool,
    /// Additional file-index roots (default: Documents/Desktop/Downloads).
    #[serde(default)]
    pub index_dirs: Vec<String>,
    /// Maximum number of result rows displayed (P2-D; clamped to
    /// `launcher_core::MAX_RESULTS` upstream).
    #[serde(default = "default_result_limit")]
    pub result_limit: usize,
    /// P2.1-D: portable app discovery roots (bounded scan; NEVER the whole
    /// disk). Empty = portable discovery off.
    #[serde(default)]
    pub portable_roots: Vec<String>,
    /// P2.1-B: watch configured index roots for filesystem changes (the
    /// IndexCoordinator replaces the periodic background rebuild).
    #[serde(default = "default_watch_enabled")]
    pub watch_enabled: bool,
    /// Python interpreter for `runtime.type = "python"` plugins. Supports
    /// environment variables (`%VAR%`, `${VAR}`, `$VAR`), e.g.
    /// `"%LOCALAPPDATA%\Programs\Python\python.exe"`.
    /// Resolution order (ADR-0009): this field > `LAUNCHER_PYTHON` env >
    /// `"python"` on PATH.
    #[serde(default)]
    pub python_path: Option<String>,
    /// Configured MCP servers (MVP4.3, Spec section 21).
    #[serde(rename = "mcp.servers", default)]
    pub mcp_servers: Vec<McpServerConfig>,
}

fn default_result_limit() -> usize {
    12
}

fn default_watch_enabled() -> bool {
    true
}

fn default_hotkey() -> String {
    "Ctrl+Space".to_string()
}

fn default_theme_color() -> String {
    "#4DA3FF".to_string()
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey: default_hotkey(),
            theme_color: default_theme_color(),
            autostart: false,
            index_dirs: Vec::new(),
            result_limit: default_result_limit(),
            watch_enabled: default_watch_enabled(),
            portable_roots: Vec::new(),
            python_path: None,
            mcp_servers: Vec::new(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("config path unavailable: {0}")]
    PathUnavailable(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

fn appdata() -> Result<PathBuf, ConfigError> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
        .ok_or_else(|| ConfigError::PathUnavailable("APPDATA not set".into()))
}

/// Config file path `%APPDATA%\NativeLauncher\config.toml`.
pub fn config_path() -> Result<PathBuf, ConfigError> {
    Ok(appdata()?.join("NativeLauncher").join("config.toml"))
}

/// Log directory `%APPDATA%\NativeLauncher\logs`.
pub fn logs_dir() -> Result<PathBuf, ConfigError> {
    Ok(appdata()?.join("NativeLauncher").join("logs"))
}

/// Accept BOTH spellings of the MCP server list in config files: the
/// documented natural TOML form `[[mcp.servers]]` (table `mcp`, key
/// `servers`) and the serializer's quoted-literal-key form
/// `[[ "mcp.servers" ]]`. Without normalization the natural form parsed to
/// ZERO servers silently (release gate finding, review 48 §15/§16).
fn normalize_mcp_tables(raw: &str) -> String {
    let mut v: toml::Value = match toml::from_str(raw) {
        Ok(v) => v,
        Err(_) => return raw.to_string(), // leave malformed input to the caller's error path
    };
    let servers = v.get("mcp").and_then(|m| m.get("servers")).cloned();
    if let (Some(servers), Some(table)) = (servers, v.as_table_mut()) {
        table.remove("mcp");
        table.insert("mcp.servers".to_string(), servers);
    }
    toml::to_string(&v).unwrap_or_else(|_| raw.to_string())
}

/// Load config: missing -> write defaults; partial -> fill defaults;
/// unparsable -> defaults + WARN. Always yields a usable config.
pub fn load_or_create(path: &Path) -> Result<AppConfig, ConfigError> {
    if !path.exists() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let default = AppConfig::default();
        let toml_text = toml::to_string_pretty(&default)
            .map_err(|e| ConfigError::PathUnavailable(format!("serialize default config: {e}")))?;
        std::fs::write(path, toml_text)?;
        return Ok(default);
    }

    let raw = match std::fs::read_to_string(path) {
        Ok(raw) => raw,
        Err(e) => {
            warn!(error = %e, path = %path.display(), "failed to read config file, using defaults");
            return Ok(AppConfig::default());
        }
    };

    match toml::from_str::<AppConfig>(&normalize_mcp_tables(&raw)) {
        Ok(cfg) => Ok(cfg),
        Err(e) => {
            // Review 64 §21: never silently lose the user's settings. The
            // broken file is quarantined with a timestamp and defaults are
            // written fresh, so the failure is visible and recoverable.
            warn!(error = %e, path = %path.display(), "invalid config file, quarantining and writing defaults");
            let ts = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            let quarantined = path.with_extension(format!("invalid-{ts}"));
            if std::fs::rename(path, &quarantined).is_ok() {
                warn!(quarantined = %quarantined.display(), "broken config preserved");
            }
            let default = AppConfig::default();
            let _ = save(path, &default);
            Ok(default)
        }
    }
}

/// Save config ATOMICALLY (review 64 §21): write to a sibling temp file,
/// flush it to disk, then rename over the target — a crash mid-save can
/// never leave a half-written config behind (the previous direct
/// `std::fs::write` could, and a corrupt config meant silent defaults).
pub fn save(path: &Path, cfg: &AppConfig) -> Result<(), ConfigError> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let toml_text = toml::to_string_pretty(cfg)
        .map_err(|e| ConfigError::PathUnavailable(format!("serialize config: {e}")))?;
    let tmp = path.with_extension("toml.tmp");
    {
        use std::io::Write;
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(toml_text.as_bytes())?;
        f.flush()?;
        f.sync_all()?;
    }
    // rename-over is atomic on the same volume (Windows: ReplaceFileW
    // semantics via std::fs; a leftover .tmp on failure is harmless)
    std::fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TempFile(PathBuf);

    impl TempFile {
        fn new(tag: &str) -> Self {
            let p = std::env::temp_dir().join(format!(
                "launcher2-cfg-test-{}-{}-{}.toml",
                tag,
                std::process::id(),
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos())
                    .unwrap_or(0)
            ));
            TempFile(p)
        }
    }

    impl Drop for TempFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }

    #[test]
    fn missing_file_creates_defaults_and_reads_back() {
        let t = TempFile::new("missing");
        let cfg = load_or_create(&t.0).expect("load on missing file should not fail");
        assert_eq!(cfg, AppConfig::default());
        assert!(t.0.exists(), "default config file should be written");
        let cfg2 = load_or_create(&t.0).expect("reload should not fail");
        assert_eq!(cfg2, cfg);
    }

    #[test]
    fn missing_fields_filled_with_defaults() {
        let t = TempFile::new("partial");
        std::fs::write(&t.0, "hotkey = \"Ctrl+P\"\n").unwrap();
        let cfg = load_or_create(&t.0).unwrap();
        assert_eq!(cfg.hotkey, "Ctrl+P");
        assert_eq!(cfg.theme_color, default_theme_color());
        assert!(!cfg.autostart);
    }

    #[test]
    fn invalid_toml_falls_back_to_defaults() {
        let t = TempFile::new("invalid");
        std::fs::write(&t.0, "hotkey = [not valid toml").unwrap();
        let cfg = load_or_create(&t.0).expect("invalid TOML must not crash");
        assert_eq!(cfg, AppConfig::default());
    }

    #[test]
    fn roundtrip_save_load() {
        let t = TempFile::new("roundtrip");
        let cfg = AppConfig {
            hotkey: "Ctrl+;".into(),
            theme_color: "#123456".into(),
            autostart: true,
            index_dirs: vec!["D:\\docs".into()],
            result_limit: 12,
            watch_enabled: true,
            portable_roots: Vec::new(),
            python_path: None,
            mcp_servers: Vec::new(),
        };
        save(&t.0, &cfg).unwrap();
        let loaded = load_or_create(&t.0).unwrap();
        assert_eq!(loaded, cfg);
    }

    // ---- Phase 12 release gates: configuration migration (review 48 §16) ----

    /// Gate: pre-MVP4.3 config (no mcp section) parses cleanly.
    #[test]
    fn release_config_migration_no_mcp_section() {
        let t = TempFile::new("no-mcp");
        std::fs::write(&t.0, "hotkey = \"Alt+Space\"
index_dirs = []
").unwrap();
        let cfg = load_or_create(&t.0).expect("old config must load");
        assert!(cfg.mcp_servers.is_empty());
        assert_eq!(cfg.hotkey, "Alt+Space");
    }

    /// Gate: empty/edge mcp sections are valid.
    #[test]
    fn release_config_migration_empty_and_edge_mcp() {
        let t = TempFile::new("edge-mcp");
        std::fs::write(
            &t.0,
            "[[mcp.servers]]
".to_string() + // present but missing required fields -> localized error, defaults
                "id = \"x\"
",
        )
        .unwrap();
        // malformed server entry falls back to defaults, never a startup crash
        let cfg = load_or_create(&t.0).expect("malformed mcp entry must not crash");
        assert_eq!(cfg, AppConfig::default());

        let t2 = TempFile::new("empty-array");
        std::fs::write(&t2.0, "mcp.servers = []
").unwrap();
        let cfg2 = load_or_create(&t2.0).unwrap();
        assert!(cfg2.mcp_servers.is_empty());
    }

    /// Gate: unknown top-level fields and unknown mcp server fields follow
    /// the existing policy (tolerated / defaulted), and the `profile` field
    /// round-trips both accepted spellings.
    #[test]
    fn release_config_migration_unknown_fields_and_profiles() {
        let t = TempFile::new("unknown");
        std::fs::write(
            &t.0,
            "hotkey = \"Ctrl+Space\"
unknown_future = 1
[[mcp.servers]]
id = \"c\"
program = \"p\"
profile = \"2026-07-28\"
",
        )
        .unwrap();
        let cfg = load_or_create(&t.0).unwrap();
        assert_eq!(cfg.mcp_servers.len(), 1);
        assert_eq!(cfg.mcp_servers[0].profile, "2026-07-28");

        // default profile is the legacy compatibility profile
        let t2 = TempFile::new("default-profile");
        std::fs::write(
            &t2.0,
            "[[mcp.servers]]
id = \"c\"
program = \"p\"
",
        )
        .unwrap();
        let cfg2 = load_or_create(&t2.0).unwrap();
        assert_eq!(cfg2.mcp_servers[0].profile, "");
    }

    /// Gate: the DOCUMENTED natural TOML form `[[mcp.servers]]` must load
    /// servers (release-gate finding: it silently parsed to zero before).
    #[test]
    fn release_config_natural_mcp_table_form_loads() {
        let t = TempFile::new("natural");
        std::fs::write(
            &t.0,
            "[[mcp.servers]]
id = \"calc\"
program = \"mcp-calc\"
args = [\"--x\"]
profile = \"2026-07-28\"
",
        )
        .unwrap();
        let cfg = load_or_create(&t.0).unwrap();
        assert_eq!(cfg.mcp_servers.len(), 1, "natural [[mcp.servers]] form must not be dropped");
        assert_eq!(cfg.mcp_servers[0].id, "calc");
        assert_eq!(cfg.mcp_servers[0].profile, "2026-07-28");
    }

    /// Gate: roundtrip WITH a configured server (the serializer's quoted
    /// key form must reload identically).
    #[test]
    fn release_config_roundtrip_with_mcp_server() {
        let t = TempFile::new("roundtrip-mcp");
        let cfg = AppConfig {
            mcp_servers: vec![McpServerConfig {
                id: "calc".into(),
                transport: "stdio".into(),
                program: "mcp-calc".into(),
                args: vec![],
                profile: String::new(),
                url: None,
                allow_private_network: false,
                allow_plain_http: false,
                runtime: String::new(),
            }],
            ..AppConfig::default()
        };
        save(&t.0, &cfg).unwrap();
        assert_eq!(load_or_create(&t.0).unwrap(), cfg);
    }
}