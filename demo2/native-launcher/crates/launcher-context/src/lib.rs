//! Context Engine (design spec 6): produces ContextSnapshots with
//! timestamps and confidence. MVP reads foreground app / clipboard via a
//! pluggable provider so it stays testable without a real desktop.

use std::time::{SystemTime, UNIX_EPOCH};

use launcher_domain::{ContextMeta, ContextSnapshot};

/// How old a snapshot may be before it is considered stale.
pub const STALE_AFTER_MS: u64 = 2000;

/// Source of raw OS signals; implemented for real Windows in the app layer,
/// faked in tests.
pub trait ContextSource: Send + Sync {
    fn foreground_app(&self) -> Option<String>;
    fn clipboard_kind(&self) -> Option<String>;
}

pub struct NoopSource;

impl ContextSource for NoopSource {
    fn foreground_app(&self) -> Option<String> {
        None
    }
    fn clipboard_kind(&self) -> Option<String> {
        None
    }
}

pub struct ContextEngine<S: ContextSource> {
    source: S,
}

impl<S: ContextSource> ContextEngine<S> {
    pub fn new(source: S) -> Self {
        Self { source }
    }

    pub fn snapshot(&self, current_folder: Option<String>) -> ContextSnapshot {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        let fg = self.source.foreground_app();
        let clip = self.source.clipboard_kind();
        let mut signals = 0;
        let mut known = 0;
        known += 1;
        if fg.is_some() {
            signals += 1;
        }
        known += 1;
        if clip.is_some() {
            signals += 1;
        }
        let confidence = if known == 0 {
            0.0
        } else {
            signals as f32 / known as f32
        };
        ContextSnapshot {
            foreground_app: fg,
            current_folder,
            selected_items: vec![],
            clipboard_type: clip,
            meta: Some(ContextMeta {
                timestamp_ms: now,
                confidence,
            }),
        }
    }
}

/// True when the snapshot is too old to be trusted.
pub fn is_stale(snapshot: &ContextSnapshot, now_ms: u64) -> bool {
    match &snapshot.meta {
        Some(m) => now_ms.saturating_sub(m.timestamp_ms) > STALE_AFTER_MS,
        None => true,
    }
}

pub mod explorer;

#[cfg(windows)]
pub mod windows_source {
    //! Real OS context source (v0.1.x): foreground process name via Win32.
    //! Explorer current-folder detection (IShellWindows COM) is the next step.

    use super::ContextSource;

    /// Foreground app source: process name of the foreground window.
    pub struct WindowsForegroundSource;

    /// Foreground top-level window handle (0 when unavailable).
    pub fn foreground_hwnd() -> isize {
        unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow().0 as isize }
    }

    impl WindowsForegroundSource {
        pub fn new() -> Self {
            Self
        }
    }

    impl Default for WindowsForegroundSource {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ContextSource for WindowsForegroundSource {
        fn foreground_app(&self) -> Option<String> {
            let hwnd = unsafe { windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow() };
            if hwnd.is_invalid() {
                return None;
            }
            let mut pid = 0u32;
            unsafe {
                windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(
                    hwnd,
                    Some(&mut pid),
                )
            };
            if pid == 0 {
                return None;
            }
            let handle = unsafe {
                windows::Win32::System::Threading::OpenProcess(
                    windows::Win32::System::Threading::PROCESS_QUERY_LIMITED_INFORMATION,
                    false,
                    pid,
                )
            }
            .ok()?;
            let mut buf = [0u16; 260];
            let mut len = buf.len() as u32;
            let ok = unsafe {
                windows::Win32::System::Threading::QueryFullProcessImageNameW(
                    handle,
                    windows::Win32::System::Threading::PROCESS_NAME_FORMAT(0),
                    windows::core::PWSTR(buf.as_mut_ptr()),
                    &mut len,
                )
            };
            unsafe {
                let _ = windows::Win32::Foundation::CloseHandle(handle);
            }
            if ok.is_err() || len == 0 {
                return None;
            }
            Some(normalize_process_name(&String::from_utf16_lossy(
                &buf[..len as usize],
            )))
        }

        fn clipboard_kind(&self) -> Option<String> {
            None // clipboard integration is P1, not wired yet
        }
    }

    /// Pure helper: full path -> lowercased file name ("C:\\x\\app.exe" ->
    /// "app.exe"). Unit-testable without a desktop.
    pub fn normalize_process_name(path: &str) -> String {
        path.rsplit(['\\', '/'])
            .next()
            .unwrap_or(path)
            .to_lowercase()
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn process_name_normalization() {
            assert_eq!(normalize_process_name(r"C:\x\App.EXE"), "app.exe");
            assert_eq!(normalize_process_name("explorer.exe"), "explorer.exe");
            assert_eq!(normalize_process_name("/usr/bin/app"), "app");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct FakeSource {
        fg: Option<String>,
        clip: Option<String>,
    }

    impl ContextSource for FakeSource {
        fn foreground_app(&self) -> Option<String> {
            self.fg.clone()
        }
        fn clipboard_kind(&self) -> Option<String> {
            self.clip.clone()
        }
    }

    #[test]
    fn snapshot_captures_signals_and_confidence() {
        let e = ContextEngine::new(FakeSource {
            fg: Some("explorer.exe".into()),
            clip: None,
        });
        let s = e.snapshot(Some("C:\\Projects".into()));
        assert_eq!(s.foreground_app.as_deref(), Some("explorer.exe"));
        assert_eq!(s.current_folder.as_deref(), Some("C:\\Projects"));
        assert!(s.clipboard_type.is_none());
        let m = s.meta.unwrap();
        assert!((m.confidence - 0.5).abs() < 1e-6);
    }

    #[test]
    fn missing_foreground_window_is_ok() {
        let e = ContextEngine::new(FakeSource {
            fg: None,
            clip: None,
        });
        let s = e.snapshot(None);
        assert!(s.foreground_app.is_none());
        assert!((s.meta.unwrap().confidence - 0.0).abs() < 1e-6);
    }

    #[test]
    fn staleness() {
        let e = ContextEngine::new(FakeSource {
            fg: None,
            clip: None,
        });
        let s = e.snapshot(None);
        let now = s.meta.as_ref().unwrap().timestamp_ms;
        assert!(!is_stale(&s, now));
        assert!(is_stale(&s, now + STALE_AFTER_MS + 1));
        // snapshot without metadata is always stale
        let mut s2 = e.snapshot(None);
        s2.meta = None;
        assert!(is_stale(&s2, now));
    }
}
