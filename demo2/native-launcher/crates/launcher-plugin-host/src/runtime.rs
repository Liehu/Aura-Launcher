//! Runtime resolution (ADR-0010): the single place that maps a manifest's
//! `runtime.type` — a **Host launch strategy**, not a programming language —
//! to a concrete program + argv.
//!
//! Plugin Contract §29.19: new runtimes (node, wasm, ...) are added HERE and
//! nowhere else; `PluginHandle::spawn` stays runtime-agnostic. Interpreter
//! discovery follows the Runtime Discovery policy (config > env override >
//! system PATH); the host app resolves the interpreter and injects it via
//! `PluginManifest::interpreter`, this module only expands env vars and
//! enforces script confinement.

use launcher_domain::PluginManifest;

use super::{expand_env, PluginError};

/// How the host launches one plugin (result of resolution).
#[derive(Debug, Clone, PartialEq)]
pub struct LaunchPlan {
    /// `process` | `python` — kept for diagnostics/logging.
    pub kind: &'static str,
    /// Program to spawn (env-expanded).
    pub program: String,
    /// Full argv (script path included for script runtimes; confinement-
    /// checked against the plugin directory).
    pub args: Vec<String>,
    /// Where the program resolution came from ("config" / "env:VAR" / "path"
    /// for script runtimes; "manifest" for native executables). Closes the
    /// diagnostics chain User Config → Discovery → Resolution → Launch (ADR-0010).
    pub resolved_from: String,
}

/// Resolve `manifest.runtime` into a concrete launch plan (ADR-0010: the
/// single runtime extension point).
pub fn resolve_launch(
    manifest: &PluginManifest,
    base_dir: &std::path::Path,
) -> Result<LaunchPlan, PluginError> {
    let kind = manifest
        .runtime
        .as_ref()
        .map(|rt| rt.kind.as_str())
        .unwrap_or("process");
    match kind {
        "process" => {
            let exe = super::resolve_executable(base_dir, manifest.effective_executable())?;
            Ok(LaunchPlan {
                kind: "process",
                program: exe.to_string_lossy().into_owned(),
                args: manifest.spawn_args().to_vec(),
                resolved_from: "manifest".into(),
            })
        }
        "python" => {
            // interpreter: host-resolved (config / LAUNCHER_PYTHON / PATH),
            // env-expanded here; script stays confinement-checked inside the
            // plugin directory (INV-013).
            let interpreter = manifest
                .interpreter
                .as_ref()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|| "python".to_string());
            let script = super::resolve_executable(base_dir, manifest.effective_executable())?;
            let mut args = vec![script.to_string_lossy().into_owned()];
            args.extend(manifest.spawn_args().iter().cloned());
            Ok(LaunchPlan {
                kind: "python",
                program: expand_env(&interpreter),
                args,
                resolved_from: manifest
                    .interpreter_source
                    .clone()
                    .unwrap_or_else(|| "unspecified".into()),
            })
        }
        other => Err(PluginError::Malformed(format!(
            "unsupported runtime.type: {other}"
        ))),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use launcher_domain::RuntimeSpec;
    use std::path::PathBuf;

    fn manifest(kind: &str, executable: &str) -> PluginManifest {
        PluginManifest {
            id: "t".into(),
            name: "T".into(),
            version: None,
            api_version: "0.1".into(),
            schema_version: Some(1),
            executable: String::new(),
            runtime: Some(RuntimeSpec {
                kind: kind.into(),
                executable: Some(executable.into()),
                args: vec![],
            }),
            capabilities: vec![],
            timeout_ms: 2000,
            idle_timeout_ms: 10_000,
            interpreter: None,
            interpreter_source: None,
        }
    }

    #[test]
    fn process_plan_resolves_relative_executable() {
        let base = std::env::temp_dir().join("runtime-process");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("p.exe"), b"MZ").unwrap();
        let plan = resolve_launch(&manifest("process", "p.exe"), &base).unwrap();
        assert_eq!(plan.kind, "process");
        assert!(plan.program.ends_with("p.exe"));
        assert_eq!(plan.resolved_from, "manifest");
        assert!(plan.args.is_empty());
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn python_plan_prepends_interpreter_and_expands_env() {
        let base = std::env::temp_dir().join("runtime-python");
        std::fs::create_dir_all(&base).unwrap();
        std::fs::write(base.join("m.py"), b"#").unwrap();
        let mut m = manifest("python", "m.py");
        m.interpreter = Some(PathBuf::from("%LAUNCHER_TEST_INTERP%"));
        std::env::set_var("LAUNCHER_TEST_INTERP", r"C:\py\python.exe");
        let plan = resolve_launch(&m, &base).unwrap();
        assert_eq!(plan.kind, "python");
        assert_eq!(plan.program, r"C:\py\python.exe");
        assert_eq!(plan.resolved_from, "unspecified");
        assert_eq!(plan.args.len(), 1);
        assert!(plan.args[0].ends_with("m.py"));
        std::env::remove_var("LAUNCHER_TEST_INTERP");
        std::fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn unknown_runtime_kind_is_rejected() {
        let base = std::env::temp_dir();
        assert!(resolve_launch(&manifest("wasm", "x.wasm"), &base).is_err());
    }
}
