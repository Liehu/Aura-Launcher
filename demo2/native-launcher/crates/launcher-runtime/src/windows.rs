//! Windows process control (review 53 §9/§10): Job Object wrapper migrated
//! from the plugin host (ADR-0005). The spawn sequence keeps the
//! spawn-to-assign race closed: the process is created CREATE_SUSPENDED,
//! assigned to its kill-on-close job, and only then resumed — so the
//! process tree is inside the job from its very first instruction.

#![cfg(windows)]

use std::io;

use windows::Win32::Foundation::HANDLE;
use windows::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JobObjectExtendedLimitInformation,
    SetInformationJobObject, TerminateJobObject, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
};

#[derive(Debug)]
pub struct JobObject {
    handle: HANDLE,
}

/// Win32 CREATE_SUSPENDED.
pub const CREATE_SUSPENDED: u32 = 0x0000_0004;

/// Resume the main (first) thread of `pid` after the suspended process has
/// been assigned to its job. The process was created suspended by us, so
/// the only thread present is the main one.
pub fn resume_main_thread(pid: u32) -> io::Result<()> {
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Thread32First, Thread32Next, TH32CS_SNAPTHREAD, THREADENTRY32,
    };
    use windows::Win32::System::Threading::{OpenThread, ResumeThread, THREAD_SUSPEND_RESUME};
    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0).map_err(io::Error::from)?;
        let mut entry = THREADENTRY32 {
            dwSize: std::mem::size_of::<THREADENTRY32>() as u32,
            ..Default::default()
        };
        let mut result: io::Result<()> = Err(io::Error::new(
            io::ErrorKind::NotFound,
            "no thread found for pid",
        ));
        if Thread32First(snapshot, &mut entry).is_ok() {
            loop {
                if entry.th32OwnerProcessID == pid {
                    result = match OpenThread(THREAD_SUSPEND_RESUME, false, entry.th32ThreadID) {
                        Ok(thread) => {
                            // a fresh suspended process has suspend count 1;
                            // a single resume starts it
                            let prev = ResumeThread(thread);
                            let _ = CloseHandle(thread);
                            if prev == u32::MAX {
                                Err(io::Error::last_os_error())
                            } else {
                                Ok(())
                            }
                        }
                        Err(e) => Err(io::Error::from_raw_os_error(e.code().0)),
                    };
                    break;
                }
                if Thread32Next(snapshot, &mut entry).is_err() {
                    break;
                }
            }
        }
        let _ = CloseHandle(snapshot);
        result
    }
}

// SAFETY: the raw HANDLE is owned exclusively by this struct and all Win32
// calls operate on it; the job is only used from the thread that owns the
// session (enforced by construction), so moving it between threads is sound.
unsafe impl Send for JobObject {}

impl JobObject {
    /// Create a job with KILL_ON_JOB_CLOSE.
    pub fn create() -> io::Result<Self> {
        unsafe {
            let handle = CreateJobObjectW(None, None).map_err(io::Error::from)?;
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            SetInformationJobObject(
                handle,
                JobObjectExtendedLimitInformation,
                &limits as *const _ as *const core::ffi::c_void,
                std::mem::size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
            .map_err(io::Error::from)?;
            Ok(Self { handle })
        }
    }

    /// Assign a process (by its raw handle) to this job.
    pub fn assign(&self, process_raw_handle: std::os::windows::io::RawHandle) -> io::Result<()> {
        unsafe {
            AssignProcessToJobObject(self.handle, HANDLE(process_raw_handle as _))
                .map_err(io::Error::from)
        }
    }

    /// Kill every process in the job (process-tree containment).
    pub fn terminate(&mut self) {
        unsafe {
            let _ = TerminateJobObject(self.handle, 1);
        }
    }
}

impl Drop for JobObject {
    fn drop(&mut self) {
        self.terminate();
    }
}
