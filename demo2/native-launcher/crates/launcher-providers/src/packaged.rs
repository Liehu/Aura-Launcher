//! Packaged (MSIX/AppX/UWP) application discovery (P2.1-D, review 78 §9/§10/§42):
//! PackageManager is the AUTHORITATIVE enumeration source — WindowsApps is
//! never scanned. Identity = package FAMILY + package-local Application Id
//! (INV-APP-004/005: family is version-stable; ApplicationId is only unique
//! within a package). Launch uses the `shell:AppsFolder\<family>!<appid>`
//! alias — activation stays in the Action layer, the catalog only records it.

use std::path::PathBuf;

use tracing::warn;

use super::app_registry::AppEntry;

pub const MAX_PACKAGED: usize = 512;

/// One packaged application parsed from a package manifest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackagedApplication {
    pub package_family: String,
    pub application_id: String,
    pub display_name: String,
    /// `shell:AppsFolder\<family>!<appid>` — the launch alias.
    pub launch_alias: String,
}

/// Deterministic launch alias for a packaged application.
pub fn launch_alias(package_family: &str, application_id: &str) -> String {
    format!(r"shell:AppsFolder\{package_family}!{application_id}")
}

/// INV-APP-004/005: canonical identity key = family + application id
/// (version/architecture/publisher-id are metadata, never identity).
pub fn packaged_identity_key(package_family: &str, application_id: &str) -> String {
    format!(
        "packaged:{}:{}",
        package_family.to_lowercase(),
        application_id.to_lowercase()
    )
}

/// Extract the `<Application>` entries from an AppxManifest XML. Pure string
/// scanning (no XML crate); DisplayName may be an `ms-resource:` token — the
/// caller falls back to the package name in that case.
pub fn parse_manifest_applications(manifest: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let Some(apps_start) = manifest.find("<Applications") else {
        return out;
    };
    let apps_end = manifest[apps_start..].find("</Applications>").map(|i| apps_start + i);
    let Some(end) = apps_end else { return out };
    let scope = &manifest[apps_start..end];
    let mut rest = scope;
    while let Some(a) = rest.find("<Application ") {
        let after = &rest[a..];
        let Some(close) = after.find('>') else { break };
        let tag = &after[..=close];
        let id = attr(tag, "Id");
        let display = attr(tag, "DisplayName");
        if let Some(id) = id {
            out.push((id, display.unwrap_or_default()));
        }
        rest = &after[close..];
        if rest.len() < 2 {
            break;
        }
    }
    out
}

fn attr(tag: &str, name: &str) -> Option<String> {
    let pat = format!("{name}=\"");
    let i = tag.find(&pat)? + pat.len();
    let rest = &tag[i..];
    let j = rest.find('"')?;
    Some(rest[..j].to_string())
}

/// Deterministic display name: manifest DisplayName when it is a literal
/// string; package name when it is an ms-resource token or missing
/// (review 78 §63: simple cleanup only, no AI/network identification).
fn display_name_for(manifest_display: &str, package_name: &str) -> String {
    if manifest_display.is_empty() || manifest_display.starts_with("ms-resource:") {
        package_name.to_string()
    } else {
        manifest_display.to_string()
    }
}

/// Enumerate the current user's packaged applications. Bounded (`max`).
/// Source failures degrade to an empty list + WARN — never a panic.
#[cfg(windows)]
pub fn collect_packaged_entries(max: usize) -> Vec<AppEntry> {
    use windows::Management::Deployment::PackageManager;

    let mut out = Vec::new();
    let packages = match PackageManager::new().and_then(|pm| pm.FindPackages()) {
        Ok(p) => p,
        Err(e) => {
            warn!(error = %e, "package enumeration failed (packaged apps unavailable)");
            return out;
        }
    };
    let it = match packages.First() {
        Ok(it) => it,
        Err(e) => {
            warn!(error = %e, "package iterator failed");
            return out;
        }
    };
    while out.len() < max {
        let has_next = match it.MoveNext() {
            Ok(m) => m,
            Err(_) => break,
        };
        if !has_next {
            break;
        }
        let Ok(pkg) = it.Current() else { break };
        // framework/resource packages are libraries, not launchable apps
        let Ok(is_framework) = pkg.IsFramework() else { break };
        if is_framework {
            continue;
        }
        let Ok(id) = pkg.Id() else { continue };
        let (Ok(family_h), Ok(name_h)) = (id.FamilyName(), id.Name()) else { continue };
        let family = family_h.to_string();
        let name = name_h.to_string();
        let Ok(installed) = pkg.InstalledPath() else { continue };
        let manifest_path: PathBuf = PathBuf::from(installed.to_string()).join("AppxManifest.xml");
        let Ok(manifest) = std::fs::read_to_string(&manifest_path) else { continue };
        for (appid, display) in parse_manifest_applications(&manifest) {
            if appid.is_empty() {
                continue;
            }
            let display_name = display_name_for(&display, name.as_str());
            out.push(AppEntry {
                name: display_name,
                resolved_target: None,
                path: PathBuf::from(launch_alias(family.as_str(), &appid)),
                merged_sources: Vec::new(),
                source: "packaged",
            });
            if out.len() >= max {
                break;
            }
        }
    }
    info_packaged(out.len());
    out
}

#[cfg(windows)]
fn info_packaged(n: usize) {
    tracing::info!(packaged = n, "packaged app discovery done");
}

#[cfg(not(windows))]
pub fn collect_packaged_entries(_max: usize) -> Vec<AppEntry> {
    Vec::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app_registry::AppRegistryProvider;

    const MANIFEST: &str = r#"<Package xmlns="http://schemas.microsoft.com/appx/manifest/foundation/windows10">
  <Identity Name="Contoso.Foo" Version="1.0.0.0" Publisher="CN=Contoso" ProcessorArchitecture="x64"/>
  <Applications>
    <Application Id="Main" DisplayName="Foo Main" Executable="foo.exe"/>
    <Application Id="Settings" DisplayName="ms-resource:SettingsName" Executable="settings.exe"/>
  </Applications>
</Package>"#;

    /// P2.1-D Gate D4: one package, MULTIPLE applications — each is its own
    /// entry (Package ≠ Application, review 78 §7).
    #[test]
    fn manifest_parses_multiple_applications() {
        let apps = parse_manifest_applications(MANIFEST);
        assert_eq!(apps.len(), 2);
        assert_eq!(apps[0].0, "Main");
        assert_eq!(apps[1].0, "Settings");
    }

    /// INV-APP-004: family+appid identity is version-stable; ApplicationId
    /// alone is only package-unique and must NOT collide across packages.
    #[test]
    fn packaged_identity_version_stable_and_appid_scoped() {
        let v1 = packaged_identity_key("Contoso.Foo_8wekyb3d8bbwe", "Main");
        let v2 = packaged_identity_key("Contoso.Foo_8wekyb3d8bbwe", "Main");
        assert_eq!(v1, v2, "package version change keeps identity");
        let settings = packaged_identity_key("Contoso.Foo_8wekyb3d8bbwe", "Settings");
        assert_ne!(v1, settings, "ApplicationId is package-scoped");
        let other = packaged_identity_key("Other.Foo_8wekyb3d8bbwe", "Main");
        assert_ne!(v1, other);
    }

    /// review 78 §63: ms-resource display falls back to the package name.
    #[test]
    fn display_name_fallback() {
        assert_eq!(display_name_for("Foo Main", "Contoso.Foo"), "Foo Main");
        assert_eq!(display_name_for("ms-resource:SettingsName", "Contoso.Foo"), "Contoso.Foo");
        assert_eq!(display_name_for("", "Contoso.Foo"), "Contoso.Foo");
    }

    /// Launch alias is deterministic and scoped by family+appid.
    #[test]
    fn launch_alias_scoped() {
        assert_eq!(
            launch_alias("A_b", "Main"),
            launch_alias("A_b", "Main")
        );
        assert_ne!(launch_alias("A_b", "Main"), launch_alias("A_b", "Settings"));
    }

    /// Packaged entries flow through the provider without being merged into
    /// unrelated Win32 entries.
    #[test]
    fn packaged_entries_render_as_commands() {
        let e = AppEntry {
            name: "Calculator".into(),
            resolved_target: None,
            merged_sources: Vec::new(),
            path: PathBuf::from(launch_alias("Calc_8wekyb3d8bbwe", "App")),
            source: "packaged",
        };
        let provider = AppRegistryProvider::from_entries(vec![e]);
        let cmds = provider.to_commands();
        assert_eq!(cmds.len(), 1);
        assert_eq!(cmds[0].actions[0].kind, launcher_domain::ActionKind::Open);
    }
}

#[cfg(all(windows, test))]
mod live_tests {
    use super::*;

    /// Live integration (P2.1-D Gate D3): the real PackageManager enumerates
    /// installed packages and we derive launchable entries from their
    /// manifests. Asserts a non-trivial catalog with well-formed aliases.
    #[test]
    fn packaged_discovery_finds_installed_apps() {
        let entries = collect_packaged_entries(512);
        assert!(!entries.is_empty(), "this machine has installed packaged apps");
        for e in &entries {
            let alias = e.path.to_string_lossy();
            assert!(alias.starts_with(r"shell:AppsFolder\"));
            assert!(alias.contains('!'), "alias must be family!appid");
            assert_eq!(e.source, "packaged");
        }
    }
}
