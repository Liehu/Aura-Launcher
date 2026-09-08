//! WindowsWatcher (P2.1-B, review 76 §40): the ONLY module that touches
//! ReadDirectoryChangesW. It knows nothing about SQLite, the index, search
//! or generation — its sole output is `FileChange` hints plus overflow
//! signals (review 76 §1: "watcher event is not fact").
//!
//! Overflow (ERROR_NOTIFY_ENUM_DIR, or the documented zero-bytes-completed
//! signal) surfaces as `CoordinatorMsg::Overflow { root }` → DirtyRoot →
//! bounded subtree rescan (INV-INDEX-002: never a full-disk rebuild).

use std::path::Path;
use std::sync::mpsc::Sender;

use launcher_domain::{FileChange, FileChangeKind};

#[derive(Debug)]
pub enum CoordinatorMsg {
    Change(FileChange),
    /// Buffer overflow for this root: queued events are untrustworthy; the
    /// containing root must be marked dirty and rescanned.
    Overflow { root: String },
    Stop,
}

pub struct WindowsWatcher;

impl WindowsWatcher {
    /// Spawn one watcher thread per root. Each thread runs a blocking
    /// overlapped RDC loop until a `Stop` is sent (or the handle dies).
    pub fn spawn(roots: &[std::path::PathBuf], tx: Sender<CoordinatorMsg>, stop: &StopHandle) {
        for root in roots {
            let root = root.clone();
            let tx = tx.clone();
            let stop = stop.clone();
            std::thread::Builder::new()
                .name(format!("watcher({})", root.display()))
                .spawn(move || {
                    if let Err(e) = watch_root(&root, &tx, &stop) {
                        eprintln!("[diag] watcher exited: root={} error={}", root.display(), e);
                        let _ = tx.send(CoordinatorMsg::Overflow {
                            root: root.to_string_lossy().to_string(),
                        });
                    }
                })
                .ok();
        }
    }
}

/// Cloneable stop switch: any clone signals all watcher loops to cancel.
#[derive(Clone)]
pub struct StopHandle {
    #[cfg(windows)]
    event: std::sync::Arc<StopEvent>,
    #[cfg(not(windows))]
    _p: (),
}

#[cfg(windows)]
struct StopEvent {
    event: windows::Win32::Foundation::HANDLE,
}
// HANDLE is an opaque pointer; the event is only used via
// SetEvent/WaitForSingleObject, which are thread-safe.
#[cfg(windows)]
unsafe impl Send for StopEvent {}
#[cfg(windows)]
unsafe impl Sync for StopEvent {}

#[cfg(windows)]
impl Drop for StopEvent {
    fn drop(&mut self) {
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(self.event);
        }
    }
}

impl StopHandle {
    pub fn new() -> std::io::Result<Self> {
        #[cfg(windows)]
        {
            use windows::Win32::System::Threading::CreateEventW;
            let event = unsafe { CreateEventW(None, true, false, None) }?;
            Ok(Self {
                event: std::sync::Arc::new(StopEvent { event }),
            })
        }
        #[cfg(not(windows))]
        {
            Ok(Self { _p: () })
        }
    }

    #[cfg(windows)]
    pub fn signal(&self) {
        use windows::Win32::System::Threading::SetEvent;
        unsafe {
            let _ = SetEvent(self.event.event);
        }
    }

    #[cfg(not(windows))]
    pub fn signal(&self) {}
}

impl Default for StopHandle {
    fn default() -> Self {
        Self::new().expect("CreateEventW")
    }
}

#[cfg(windows)]
fn watch_root(root: &Path, tx: &Sender<CoordinatorMsg>, stop: &StopHandle) -> windows::core::Result<()> {
    use windows::core::HSTRING;
    use windows::Win32::Foundation::{CloseHandle, ERROR_OPERATION_ABORTED, WAIT_OBJECT_0, WAIT_TIMEOUT};
    use windows::Win32::Storage::FileSystem::{
        self as fs, CreateFileW, ReadDirectoryChangesW,
        FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OVERLAPPED, FILE_LIST_DIRECTORY,
        FILE_NOTIFY_CHANGE, FILE_NOTIFY_CHANGE_ATTRIBUTES, FILE_NOTIFY_CHANGE_CREATION,
        FILE_NOTIFY_CHANGE_DIR_NAME, FILE_NOTIFY_CHANGE_FILE_NAME, FILE_NOTIFY_CHANGE_LAST_WRITE,
        FILE_NOTIFY_CHANGE_SIZE, FILE_NOTIFY_INFORMATION, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, FILE_ACTION_ADDED, FILE_ACTION_MODIFIED, FILE_ACTION_REMOVED,
        FILE_ACTION_RENAMED_NEW_NAME, FILE_ACTION_RENAMED_OLD_NAME,
    };
    use windows::Win32::System::IO::{CancelIoEx, GetOverlappedResult, OVERLAPPED};
    use windows::Win32::System::Threading::{CreateEventW, ResetEvent, WaitForMultipleObjects};
    const WAIT_IO: usize = 0;
    const WAIT_STOP: usize = 1;

    let dir = unsafe {
        CreateFileW(
            &HSTRING::from(root.as_os_str()),
            FILE_LIST_DIRECTORY.0,
            FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE,
            None,
            fs::OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OVERLAPPED,
            None,
        )?
    };
    tracing::info!(root = %root.display(), "watcher.active");

    const BUFFER_SIZE: u32 = 64 * 1024;
    let mut buffer = vec![0u8; BUFFER_SIZE as usize];
    let notify = FILE_NOTIFY_CHANGE(
        FILE_NOTIFY_CHANGE_FILE_NAME.0
            | FILE_NOTIFY_CHANGE_DIR_NAME.0
            | FILE_NOTIFY_CHANGE_LAST_WRITE.0
            | FILE_NOTIFY_CHANGE_SIZE.0
            | FILE_NOTIFY_CHANGE_ATTRIBUTES.0
            | FILE_NOTIFY_CHANGE_CREATION.0,
    );

    let mut pending_old: Option<String> = None;
    // SEPARATE events: IO completion (auto-reset) vs stop (manual reset).
    // Sharing one manual-reset event made the first completion look like a
    // stop and silently killed the watcher (found by B10 E2E).
    let io_event = unsafe { CreateEventW(None, false, false, None) }?;
    let mut ov = OVERLAPPED::default();
    ov.hEvent = io_event;
    let handles = [io_event, stop.event.event];

    let mut issued = false;
    loop {
        if !issued {
            unsafe {
                ReadDirectoryChangesW(
                    dir,
                    buffer.as_mut_ptr().cast(),
                    BUFFER_SIZE,
                    true, // watch subtree (reparse targets are NOT re-rooted)
                    notify,
                    None,
                    Some(&mut ov),
                    None,
                )?;
            }
            issued = true;
        }
        // wait: IO completion, stop signal, or the 500ms tick
        let w = unsafe {
            WaitForMultipleObjects(&handles, false, 500)
        };
        let w = w.0;
        match w {
            _ if w as usize == WAIT_OBJECT_0.0 as usize + WAIT_STOP => {
                // stop signalled: cancel pending IO and exit
                unsafe {
                    let _ = CancelIoEx(dir, Some(&ov));
                    let mut bytes: u32 = 0;
                    let _ = GetOverlappedResult(dir, &ov, &mut bytes, true);
                    let _ = CloseHandle(io_event);
                }
                let _ = tx.send(CoordinatorMsg::Stop);
                return Ok(());
            }
            _ if w as usize == WAIT_OBJECT_0.0 as usize + WAIT_IO => {}
            _ if w as usize == WAIT_TIMEOUT.0 as usize => continue,
            _ => continue,
        }
        let mut bytes: u32 = 0;
        let res = unsafe { GetOverlappedResult(dir, &ov, &mut bytes, false) };
        issued = false; // re-issue after processing regardless of outcome
        unsafe {
            let _ = ResetEvent(io_event);
        }
        match res {
            Err(e) if e.code() == windows::Win32::Foundation::ERROR_NOTIFY_ENUM_DIR.into() => {
                // ERROR_NOTIFY_ENUM_DIR: buffer overflow — events lost
                let _ = tx.send(CoordinatorMsg::Overflow {
                    root: root.to_string_lossy().to_string(),
                });
                continue;
            }
            Err(e) if e.code() == ERROR_OPERATION_ABORTED.into() => return Ok(()),
            Err(e) => return Err(e),
            Ok(()) => {
                if bytes == 0 {
                    // documented overflow indicator: success with 0 bytes
                    let _ = tx.send(CoordinatorMsg::Overflow {
                        root: root.to_string_lossy().to_string(),
                    });
                    continue;
                }
                // parse the FILE_NOTIFY_INFORMATION chain
                let mut offset = 0usize;
                loop {
                    let info = unsafe {
                        &*(buffer.as_ptr().add(offset) as *const FILE_NOTIFY_INFORMATION)
                    };
                    let name_len = (info.FileNameLength as usize) / 2;
                    // FileName is declared [u16; 1]; the real length is
                    // FileNameLength — slice via raw parts.
                    let name: String = unsafe {
                        String::from_utf16_lossy(std::slice::from_raw_parts(
                            info.FileName.as_ptr(),
                            name_len,
                        ))
                    };
                    let full = root.join(&name).to_string_lossy().to_string();
                    let action = info.Action;
                    match action {
                        FILE_ACTION_ADDED => {
                            let _ = tx.send(CoordinatorMsg::Change(FileChange {
                                kind: FileChangeKind::Created,
                                path: full,
                                old_path: None,
                            }));
                        }
                        FILE_ACTION_MODIFIED => {
                            let _ = tx.send(CoordinatorMsg::Change(FileChange {
                                kind: FileChangeKind::Modified,
                                path: full,
                                old_path: None,
                            }));
                        }
                        FILE_ACTION_REMOVED => {
                            let _ = tx.send(CoordinatorMsg::Change(FileChange {
                                kind: FileChangeKind::Deleted,
                                path: full,
                                old_path: None,
                            }));
                        }
                        FILE_ACTION_RENAMED_OLD_NAME => {
                            pending_old = Some(full);
                        }
                        FILE_ACTION_RENAMED_NEW_NAME => {
                            let old = pending_old.take();
                            let _ = tx.send(CoordinatorMsg::Change(FileChange {
                                kind: FileChangeKind::Renamed,
                                path: full,
                                old_path: old,
                            }));
                        }
                        _ => {}
                    }
                    if info.NextEntryOffset == 0 {
                        break;
                    }
                    offset += info.NextEntryOffset as usize;
                    if offset >= bytes as usize {
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(not(windows))]
fn watch_root(
    _root: &Path,
    _tx: &Sender<CoordinatorMsg>,
    _stop: &StopHandle,
) -> Result<(), String> {
    Err("watcher requires windows".into())
}
