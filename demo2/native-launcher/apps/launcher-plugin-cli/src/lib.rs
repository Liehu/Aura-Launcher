//! launcher-plugin CLI (P2.4-D, spec `01-P2.4-DESIGN-SPEC.md` §7): plugin
//! development tooling independent from host internals.
//!
//! Subcommands:
//!   init <dir>                     scaffold a Python plugin skeleton
//!   validate <dir>                 manifest parse + validate + entrypoint check
//!   package <dir> --out <file>     deterministic .nlpkg envelope
//!   install <pkg> --root <dir>     staged install (validate → stage → swap)
//!   uninstall <id> --root <dir>    remove an installed plugin
//!   run <dir> [--query TEXT]       dev driver over the REAL host machinery
//!   inspect <pkg|dir>              manifest + file listing
//!
//! Package format: a JSON envelope `{contract_version, manifest, files}` with
//! file bodies base64-encoded. Validation is deterministic and fails closed
//! (P2.4-D03): path traversal, duplicate ids, missing entrypoints and
//! unsupported runtimes are rejected before anything touches disk.
//!
//! `run` uses the production host spawn path (`launcher_plugin_host::
//! PluginHandle::spawn` — Job Object isolation, bounded IO, protocol
//! handshake); dev tooling never bypasses host isolation (P2.4-D05).

pub mod audit;
pub mod foundation;
pub mod repository;
pub mod resolver;
pub mod transactional;
pub mod trust;

use anyhow::{anyhow, bail, Context, Result};
use launcher_domain::PluginManifest;
use std::path::{Path, PathBuf};

pub const CONTRACT_VERSION: &str = "0.1";
const MANIFEST_NAME: &str = "plugin.json";
const MAX_PACKAGE_BYTES: usize = 32 * 1024 * 1024;

/// Entry point used by main.rs; returns process exit code.
pub fn run_cli(args: &[String]) -> i32 {
    match dispatch(args) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("error: {e:#}");
            1
        }
    }
}

fn dispatch(args: &[String]) -> Result<()> {
    let cmd = args.first().map(String::as_str).unwrap_or("");
    let rest = &args[1.min(args.len())..];
    match cmd {
        "init" => cmd_init(rest),
        "validate" => cmd_validate(rest),
        "package" => cmd_package(rest),
        "install" => cmd_install(rest),
        "uninstall" => cmd_uninstall(rest),
        "run" => cmd_run(rest),
        "replay" => cmd_replay(rest),
        "inspect" => cmd_inspect(rest),
        "help" | "--help" | "-h" | "" => {
            print_help();
            Ok(())
        }
        other => {
            bail!("unknown command `{other}` (try `launcher-plugin help`)")
        }
    }
}

fn print_help() {
    println!(
        "launcher-plugin — Native Launcher plugin dev tool (contract {CONTRACT_VERSION})

USAGE:
  launcher-plugin init <dir> [--id ID] [--name NAME]
  launcher-plugin validate <dir>
  launcher-plugin package <dir> --out <file.nlpkg>
  launcher-plugin install <file.nlpkg> --root <plugins-dir>
  launcher-plugin uninstall <plugin-id> --root <plugins-dir>
  launcher-plugin run <dir> [--query TEXT]
  launcher-plugin replay <dir> --queries <file> (one query per line)
  launcher-plugin inspect <file.nlpkg|dir>"
    );
}

fn arg_value(args: &[String], flag: &str) -> Option<String> {
    args.iter()
        .position(|a| a == flag)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

// ---- init -----------------------------------------------------------------

fn cmd_init(args: &[String]) -> Result<()> {
    let dir = PathBuf::from(
        args.first()
            .ok_or_else(|| anyhow!("init requires a target directory"))?,
    );
    let id = arg_value(args, "--id").unwrap_or_else(|| "dev.plugin".into());
    let name = arg_value(args, "--name").unwrap_or_else(|| "Dev Plugin".into());
    validate_plugin_id(&id)?;
    std::fs::create_dir_all(&dir)?;
    let manifest_path = dir.join(MANIFEST_NAME);
    if manifest_path.exists() {
        bail!("refusing to overwrite existing {}", manifest_path.display());
    }
    let manifest = serde_json::json!({
        "id": id,
        "name": name,
        "version": "0.1.0",
        "runtime": { "type": "python", "executable": "main.py" },
        "capabilities": ["clipboard.write"]
    });
    std::fs::write(&manifest_path, serde_json::to_string_pretty(&manifest)? + "\n")?;
    std::fs::write(
        dir.join("main.py"),
        format!(
            r#"# {name} - scaffolded by launcher-plugin init (SDK v0.1)
import sys
from pathlib import Path

# point at the Native Launcher Python SDK (plugins/python/launcher_plugin.py)
sys.path.insert(0, r"<path-to-plugins-python-sdk>")

from launcher_plugin import Command, Plugin  # noqa: E402


class Dev(Plugin):
    def query(self, text):
        return [Command(title=f"Echo: {{text}}", subtitle="from dev plugin")]


if __name__ == "__main__":
    Dev().run()
"#
        ),
    )?;
    println!("scaffolded plugin `{id}` in {}", dir.display());
    println!("next: launcher-plugin validate {}", dir.display());
    Ok(())
}

// ---- validate -------------------------------------------------------------

fn load_manifest(dir: &Path) -> Result<PluginManifest> {
    let path = dir.join(MANIFEST_NAME);
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("reading {}", path.display()))?;
    let manifest =
        PluginManifest::parse(&raw).map_err(|e| anyhow!("manifest parse: {}", e.0))?;
    manifest
        .validate()
        .map_err(|e| anyhow!("manifest invalid: {}", e.0))?;
    Ok(manifest)
}

fn cmd_validate(args: &[String]) -> Result<()> {
    let dir = PathBuf::from(
        args.first()
            .ok_or_else(|| anyhow!("validate requires a plugin directory"))?,
    );
    let manifest = load_manifest(&dir)?;
    // entrypoint must exist inside the plugin dir (confinement, INV-013)
    let entry = manifest.effective_executable().to_string();
    let entry_path = dir.join(&entry);
    if !entry_path.exists() {
        bail!("entrypoint missing: {}", entry_path.display());
    }
    // traversal guard: the resolved entrypoint must stay inside the dir
    ensure_inside(&dir, &entry_path)?;
    println!(
        "ok: {} v{} (runtime {}, entrypoint {})",
        manifest.id,
        manifest.version.as_deref().unwrap_or("0.0.0"),
        if manifest.is_python() { "python" } else { "process" },
        entry
    );
    Ok(())
}

/// Capability names via serde (the rename strings ARE the wire vocabulary).
fn capability_names(m: &PluginManifest) -> Vec<String> {
    m.capabilities
        .iter()
        .map(|c| {
            serde_json::to_value(c)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default()
        })
        .collect()
}

fn ensure_inside(base: &Path, candidate: &Path) -> Result<()> {
    let base = base.canonicalize().unwrap_or_else(|_| base.to_path_buf());
    let candidate = candidate.canonicalize().unwrap_or_else(|_| candidate.to_path_buf());
    if !candidate.starts_with(&base) {
        bail!("path escapes plugin directory: {}", candidate.display());
    }
    Ok(())
}

fn validate_plugin_id(id: &str) -> Result<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_');
    if !ok {
        bail!("invalid plugin id `{id}` (allowed: alnum, '.', '-', '_', <=128 chars)");
    }
    Ok(())
}

// ---- package --------------------------------------------------------------

fn b64encode(data: &[u8]) -> String {
    const T: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
        out.push(T[(n >> 18) as usize & 63] as char);
        out.push(T[(n >> 12) as usize & 63] as char);
        out.push(if chunk.len() > 1 { T[(n >> 6) as usize & 63] as char } else { '=' });
        out.push(if chunk.len() > 2 { T[n as usize & 63] as char } else { '=' });
    }
    out
}

fn b64decode(s: &str) -> Result<Vec<u8>> {
    fn val(c: u8) -> Result<u32> {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => bail!("invalid base64"),
        };
        Ok(u32::from(v))
    }
    let bytes: Vec<u8> = s.bytes().filter(|b| !b.is_ascii_whitespace() && *b != b'=').collect();
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut n: u32 = 0;
        for (i, c) in chunk.iter().enumerate() {
            n |= val(*c)? << (18 - 6 * i);
        }
        out.push((n >> 16) as u8);
        if chunk.len() > 2 {
            out.push((n >> 8) as u8);
        }
        if chunk.len() > 3 {
            out.push(n as u8);
        }
    }
    Ok(out)
}

fn collect_files(dir: &Path, rel: Option<&Path>, out: &mut Vec<(String, Vec<u8>)>) -> Result<()> {
    let base = rel.map(|r| dir.join(r)).unwrap_or_else(|| dir.to_path_buf());
    for e in std::fs::read_dir(&base)? {
        let e = e?;
        let p = e.path();
        let rel_path = rel
            .map(|r| r.join(e.file_name()))
            .unwrap_or_else(|| PathBuf::from(e.file_name()));
        let rel_str = rel_path.to_string_lossy().replace('\\', "/");
        if rel_str.starts_with('.') || rel_str.contains("/.") {
            continue; // no dotfiles/dot-dirs in packages
        }
        if p.is_dir() {
            collect_files(dir, Some(&rel_path), out)?;
        } else {
            let data = std::fs::read(&p)?;
            out.push((rel_str, data));
        }
    }
    Ok(())
}

fn cmd_package(args: &[String]) -> Result<()> {
    let dir = PathBuf::from(
        args.first()
            .ok_or_else(|| anyhow!("package requires a plugin directory"))?,
    );
    let out = arg_value(args, "--out")
        .ok_or_else(|| anyhow!("package requires --out <file.nlpkg>"))?;
    let manifest = load_manifest(&dir)?;
    let mut files = Vec::new();
    collect_files(&dir, None, &mut files)?;
    if files.is_empty() {
        bail!("package would be empty");
    }
    let total: usize = files.iter().map(|(_, d)| d.len()).sum();
    if total > MAX_PACKAGE_BYTES {
        bail!("package too large: {total} bytes > {MAX_PACKAGE_BYTES}");
    }
    let files_json: serde_json::Map<String, serde_json::Value> = files
        .into_iter()
        .map(|(name, data)| {
            (name, serde_json::Value::String(b64encode(&data)))
        })
        .collect();
    let envelope = serde_json::json!({
        "contract_version": CONTRACT_VERSION,
        "manifest": serde_json::to_value(&manifest)?,
        "files": files_json,
    });
    std::fs::write(&out, serde_json::to_vec(&envelope)?)?;
    println!("packaged {} -> {}", dir.display(), out);
    Ok(())
}

// ---- install ---------------------------------------------------------------

/// Deterministic, fail-closed validation of an envelope before it touches the
/// plugins root (P2.4-D03/D04). Rejects: missing manifest, invalid manifest,
/// path traversal in ANY packaged file name, duplicate ids, missing
/// entrypoint, oversized packages, unsupported contract versions.
fn parse_envelope(raw: &[u8]) -> Result<(PluginManifest, Vec<(String, Vec<u8>)>)> {
    if raw.len() > MAX_PACKAGE_BYTES {
        bail!("package too large");
    }
    let env: serde_json::Value =
        serde_json::from_slice(raw).context("package is not a valid envelope")?;
    let contract = env["contract_version"].as_str().unwrap_or("");
    if contract != CONTRACT_VERSION {
        bail!("unsupported contract version `{contract}` (want {CONTRACT_VERSION})");
    }
    let manifest_raw = serde_json::to_string(&env["manifest"])?;
    let manifest =
        PluginManifest::parse(&manifest_raw).map_err(|e| anyhow!("manifest: {}", e.0))?;
    manifest.validate().map_err(|e| anyhow!("manifest: {}", e.0))?;
    validate_plugin_id(&manifest.id)?;
    let entry = manifest.effective_executable().to_string();
    let files = env["files"].as_object().ok_or_else(|| anyhow!("files missing"))?;
    let mut out = Vec::new();
    for (name, body) in files {
        // traversal guard on every packaged path
        if name.starts_with('/') || name.contains('\\')
            || name.split('/').any(|seg| seg == ".." || seg == "." || seg.is_empty())
        {
            bail!("packaged file name escapes package: {name}");
        }
        let b64 = body.as_str().ok_or_else(|| anyhow!("file {name} is not a string"))?;
        let data = b64decode(b64)?;
        if *name == entry && data.is_empty() {
            bail!("entrypoint {name} is empty");
        }
        out.push((name.clone(), data));
    }
    if !out.iter().any(|(n, _)| n == &entry) {
        bail!("entrypoint {entry} missing from package");
    }
    Ok((manifest, out))
}

fn cmd_install(args: &[String]) -> Result<()> {
    let pkg = PathBuf::from(
        args.first()
            .ok_or_else(|| anyhow!("install requires a .nlpkg file"))?,
    );
    let root = PathBuf::from(
        arg_value(args, "--root")
            .ok_or_else(|| anyhow!("install requires --root <plugins-dir>"))?,
    );
    let raw = std::fs::read(&pkg).with_context(|| format!("reading {}", pkg.display()))?;
    let (manifest, files) = parse_envelope(&raw)?;
    // staged install: write to <id>.staging, then atomic swap into place
    let staging = root.join(format!("{}.staging", manifest.id));
    let target = root.join(&manifest.id);
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(&staging)?;
    for (name, data) in &files {
        let p = staging.join(name);
        if let Some(parent) = p.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(p, data)?;
    }
    // validate the STAGED copy before it becomes visible to the host
    let staged_manifest = load_manifest(&staging)?;
    if staged_manifest.id != manifest.id {
        let _ = std::fs::remove_dir_all(&staging);
        bail!("staged manifest id mismatch");
    }
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::rename(&staging, &target)?;
    println!("installed {} -> {}", manifest.id, target.display());
    Ok(())
}

fn cmd_uninstall(args: &[String]) -> Result<()> {
    let id = args
        .first()
        .ok_or_else(|| anyhow!("uninstall requires a plugin id"))?;
    validate_plugin_id(id)?;
    let root = PathBuf::from(
        arg_value(args, "--root")
            .ok_or_else(|| anyhow!("uninstall requires --root <plugins-dir>"))?,
    );
    let target = root.join(id);
    if !target.exists() {
        bail!("plugin `{id}` is not installed at {}", target.display());
    }
    std::fs::remove_dir_all(&target)?;
    println!("uninstalled {id}");
    Ok(())
}

// ---- run -------------------------------------------------------------------

fn cmd_run(args: &[String]) -> Result<()> {
    let dir = PathBuf::from(
        args.first()
            .ok_or_else(|| anyhow!("run requires a plugin directory"))?,
    );
    let query = arg_value(args, "--query").unwrap_or_else(|| "hello".into());
    let n = run_one(&dir, &query)?;
    println!("({n} result(s))");
    Ok(())
}

/// P2.4-E04: replay saved query inputs against a plugin through the same
/// host-isolated path as `run` — recorded inputs, real protocol.
fn cmd_replay(args: &[String]) -> Result<()> {
    let dir = PathBuf::from(
        args.first()
            .ok_or_else(|| anyhow!("replay requires a plugin directory"))?,
    );
    let queries_file = PathBuf::from(
        arg_value(args, "--queries")
            .ok_or_else(|| anyhow!("replay requires --queries <file> (one query per line)"))?,
    );
    let raw = std::fs::read_to_string(&queries_file)
        .with_context(|| format!("reading {}", queries_file.display()))?;
    let queries: Vec<String> =
        raw.lines().map(str::to_string).filter(|l| !l.trim().is_empty()).collect();
    if queries.is_empty() {
        bail!("replay file has no queries");
    }
    let mut total = 0;
    for (i, query) in queries.iter().enumerate() {
        // one plugin process per replayed query: the host's spawn/shutdown
        // path IS the contract under test
        let n = run_one(&dir, query).map_err(|e| anyhow!("replay[{i}] {query:?}: {e:#}"))?;
        println!("replay[{i}] {query:?} -> {n} result(s)");
        total += n;
    }
    println!("replayed {} query/queries, {total} result(s) total", queries.len());
    Ok(())
}

/// One query against the plugin through the REAL host spawn path
/// (P2.4-D05: Job Object isolation, bounded IO, protocol handshake).
fn run_one(dir: &Path, query: &str) -> Result<usize> {
    let mut manifest = load_manifest(dir)?;
    // dev convenience: allow LAUNCHER_PYTHON for python plugins (same order
    // as the host app's resolution, minus config which belongs to the app)
    if manifest.is_python() && manifest.interpreter.is_none() {
        if let Ok(py) = std::env::var("LAUNCHER_PYTHON") {
            manifest.interpreter = Some(PathBuf::from(py));
        }
    }
    let mut handle = launcher_plugin_host::PluginHandle::spawn(manifest, dir)?;
    let results = handle.query(query)?;
    for c in &results {
        println!("  {}  {}", c.id, c.title);
    }
    let n = results.len();
    handle.shutdown();
    Ok(n)
}

// ---- inspect ----------------------------------------------------------------

fn cmd_inspect(args: &[String]) -> Result<()> {
    let target = PathBuf::from(
        args.first()
            .ok_or_else(|| anyhow!("inspect requires a package or directory"))?,
    );
    if target.is_dir() {
        let manifest = load_manifest(&target)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "directory",
                "id": manifest.id,
                "name": manifest.name,
                "version": manifest.version,
                "api_version": manifest.api_version,
                "capabilities": capability_names(&manifest),
                "entrypoint": manifest.effective_executable(),
            }))?
        );
    } else {
        let raw = std::fs::read(&target)?;
        let (manifest, files) = parse_envelope(&raw)?;
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({
                "kind": "nlpkg",
                "id": manifest.id,
                "name": manifest.name,
                "version": manifest.version,
                "api_version": manifest.api_version,
                "capabilities": capability_names(&manifest),
                "entrypoint": manifest.effective_executable(),
                "files": files.iter().map(|(n, d)| {
                    serde_json::json!({"name": n, "bytes": d.len()})
                }).collect::<Vec<_>>(),
            }))?
        );
    }
    Ok(())
}
