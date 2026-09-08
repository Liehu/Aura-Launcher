//! Autostart: writes/deletes the HKCU
//! `Software\Microsoft\Windows\CurrentVersion\Run` value. Current-user scope
//! only, no admin rights needed (demo1 port).

use tracing::info;
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

const RUN_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const VALUE_NAME: &str = "NativeLauncher";

/// Enable or disable autostart. `enable=true` writes the current exe path;
/// `false` deletes it (missing value counts as success, idempotent).
pub fn set_autostart(enable: bool) -> anyhow::Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu.open_subkey_with_flags(RUN_KEY, winreg::enums::KEY_SET_VALUE)?;
    if enable {
        let exe = std::env::current_exe()?;
        let quoted = format!("\"{}\"", exe.display());
        key.set_value(VALUE_NAME, &quoted)?;
        info!(path = %exe.display(), "autostart enabled");
    } else {
        match key.delete_value(VALUE_NAME) {
            Ok(()) => info!("autostart disabled"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    /// Pure comparison logic extracted from registry semantics.
    fn is_autostart_value_for(value: &str, exe: &Path) -> bool {
        value
            .trim_matches('"')
            .eq_ignore_ascii_case(&exe.display().to_string())
    }

    #[test]
    fn autostart_value_comparison() {
        let exe = Path::new(r"C:\Apps\launcher-app.exe");
        assert!(is_autostart_value_for(r#""C:\Apps\launcher-app.exe""#, exe));
        assert!(is_autostart_value_for(r"C:\APPS\LAUNCHER-APP.EXE", exe));
        assert!(!is_autostart_value_for(r"C:\other.exe", exe));
    }
}
