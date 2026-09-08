//! Child process abstraction (review 53 §11/§13/§14/§17): one live
//! external process with bounded stdio, lifecycle state, and shutdown.
//!
//! IO model: the runtime provides bounded LINE transport on stdout (bytes
//! up to a cap — no JSON/NDJSON/protocol parsing) and a bounded diagnostics
//! sink on stderr. stdin is line-write. Protocol framing above lines and
//! all semantics belong to the protocol owners.
//!
//! Allocation-bound enforcement: stdout lines are read through a bounded
//! `take()` — an oversized line is rejected without ever being buffered
//! whole (same principle as the Phase 10 MCP frame fix).

use std::io::{BufReader, Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use crate::error::RuntimeError;
use crate::lifecycle::LifecycleState;
use crate::spec::{LaunchSpec, RuntimeLimits};
use crate::types::{ProcessTermination, RuntimeId};


#[derive(Debug)]
pub struct ProcessSession {
    pub id: RuntimeId,
    child: Child,
    stdin: std::process::ChildStdin,
    stdout_lines: Receiver<Result<String, RuntimeError>>,
    stderr: std::sync::Arc<std::sync::Mutex<Vec<u8>>>,
    stderr_truncated: std::sync::Arc<std::sync::atomic::AtomicBool>,
    state: LifecycleState,
    last_activity_at: Instant,
    limits: RuntimeLimits,
    #[cfg(windows)]
    job: Option<crate::windows::JobObject>,
}

impl ProcessSession {
    /// Spawn per `spec` under `limits`. On Windows the process is created
    /// CREATE_SUSPENDED, assigned to its kill-on-close job, and only then
    /// resumed (ADR-0005; review 53 §9) — the tree is contained from its
    /// first instruction.
    pub fn spawn(spec: &LaunchSpec, limits: &RuntimeLimits) -> Result<Self, RuntimeError> {
        if spec.program.as_os_str().is_empty() {
            return Err(RuntimeError::InvalidSpec("empty program".into()));
        }
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // stderr is a diagnostics channel; the runtime drains it under a
            // cap, it is never protocol
            .stderr(Stdio::piped());
        if let Some(dir) = &spec.working_dir {
            command.current_dir(dir);
        }
        for (k, v) in &spec.env {
            command.env(k, v);
        }
        #[cfg(windows)]
        let mut job = Some(
            crate::windows::JobObject::create().map_err(|e| RuntimeError::SpawnFailed(e.to_string()))?,
        );
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt as _;
            command.creation_flags(crate::windows::CREATE_SUSPENDED);
        }
        let mut child = command
            .spawn()
            .map_err(|e| RuntimeError::SpawnFailed(e.to_string()))?;
        #[cfg(windows)]
        if let Some(job) = job.as_mut() {
            use std::os::windows::io::AsRawHandle;
            job.assign(child.as_raw_handle())
                .map_err(|e| RuntimeError::SpawnFailed(e.to_string()))?;
            crate::windows::resume_main_thread(child.id())
                .map_err(|e| RuntimeError::SpawnFailed(e.to_string()))?;
        }

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| RuntimeError::SpawnFailed("no stdin pipe".into()))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| RuntimeError::SpawnFailed("no stdout pipe".into()))?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| RuntimeError::SpawnFailed("no stderr pipe".into()))?;

        // bounded stdout line reader (io.rs): allocation-bound, protocol-free
        let (tx, rx) = mpsc::channel();
        let cap = limits.max_stdout_line_bytes;
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stdout);
            loop {
                match crate::io::read_bounded_line(&mut reader, cap) {
                    Ok(crate::io::LineOutcome::Line(line)) => {
                        if tx.send(Ok(line)).is_err() {
                            break;
                        }
                    }
                    Ok(crate::io::LineOutcome::LimitExceeded) => {
                        if tx
                            .send(Err(RuntimeError::OutputLimitExceeded {
                                stream: "stdout",
                                limit: cap,
                            }))
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(crate::io::LineOutcome::Eof) => break,
                    Err(_) => {
                        let _ = tx.send(Err(RuntimeError::TransportBroken));
                        break;
                    }
                }
            }
        });

        // bounded stderr sink: drain always (never block the child on a
        // full diagnostics pipe), retain at most max_stderr_bytes
        let stderr_sink: std::sync::Arc<std::sync::Mutex<Vec<u8>>> =
            std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
        let truncated = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let sink = stderr_sink.clone();
        let flag = truncated.clone();
        let stderr_cap = limits.max_stderr_bytes;
        std::thread::spawn(move || {
            let mut reader = BufReader::new(stderr);
            let mut chunk = [0u8; 4096];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let mut sink = sink.lock().unwrap();
                        if sink.len() < stderr_cap {
                            let remaining = stderr_cap - sink.len();
                            sink.extend_from_slice(&chunk[..n.min(remaining)]);
                        }
                        if sink.len() >= stderr_cap && n > 0 {
                            flag.store(true, std::sync::atomic::Ordering::SeqCst);
                        }
                    }
                }
            }
        });

        Ok(Self {
            id: RuntimeId::mint(),
            child,
            stdin,
            stdout_lines: rx,
            stderr: stderr_sink,
            stderr_truncated: truncated,
            state: LifecycleState::Running,
            last_activity_at: Instant::now(),
            limits: limits.clone(),
            #[cfg(windows)]
            job,
        })
    }

    pub fn limits(&self) -> &RuntimeLimits {
        &self.limits
    }

    pub fn state(&self) -> LifecycleState {
        self.state
    }

    /// P1-A (review 56 §14): time since the last runtime activity. The
    /// RuntimeManager uses this for idle sweeps; the session only records.
    pub fn last_activity(&self) -> Instant {
        self.last_activity_at
    }

    /// Write one line to the child's stdin (frames = lines; no protocol).
    /// The runtime owns the newline terminator: callers pass the frame
    /// WITHOUT a trailing newline character.
    pub fn write_line(&mut self, line: &str) -> Result<(), RuntimeError> {
        self.last_activity_at = Instant::now();
        let line = line.strip_suffix('\n').unwrap_or(line);
        self.stdin
            .write_all(line.as_bytes())
            .and_then(|_| self.stdin.write_all(b"\n"))
            .and_then(|_| self.stdin.flush())
            .map_err(|_| RuntimeError::TransportBroken)
    }

    /// Read one bounded stdout line with an explicit deadline. Timeout,
    /// crash and pipe-close are reported distinctly (review 53 §7) — the
    /// CALLER maps them to protocol errors.
    pub fn read_line(&mut self, timeout: Duration) -> Result<String, RuntimeError> {
        self.last_activity_at = Instant::now();
        let deadline = Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(RuntimeError::IoTimeout(timeout));
            }
            match self.stdout_lines.recv_timeout(remaining) {
                Ok(Ok(line)) => return Ok(line),
                Ok(Err(e)) => return Err(e),
                Err(RecvTimeoutError::Disconnected) => {
                    // stdout closed: classify by exit status when observable
                    self.state = LifecycleState::Crashed;
                    return Err(match self.poll_termination() {
                        Some(ProcessTermination::Exited { code }) => {
                            RuntimeError::ProcessExited { code }
                        }
                        _ => RuntimeError::TransportBroken,
                    });
                }
                Err(RecvTimeoutError::Timeout) => return Err(RuntimeError::IoTimeout(timeout)),
            }
        }
    }

    /// Give a just-closed stdout a brief window to observe the real exit
    /// status (review 53 §18 crash detection).
    fn poll_termination(&mut self) -> Option<ProcessTermination> {
        for _ in 0..20 {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    return Some(ProcessTermination::from_exit_code(status.code()));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => return Some(ProcessTermination::Unknown),
            }
        }
        Some(ProcessTermination::Unknown)
    }

    /// Non-blocking exit check (crash detection, review 53 §25 RT-009).
    pub fn try_termination(&mut self) -> Option<ProcessTermination> {
        match self.child.try_wait() {
            Ok(Some(status)) => {
                self.state = LifecycleState::Stopped;
                Some(ProcessTermination::from_exit_code(status.code()))
            }
            Ok(None) => None,
            Err(_) => Some(ProcessTermination::Unknown),
        }
    }

    /// Diagnostics snapshot (bounded; never protocol data).
    pub fn stderr_snapshot(&self) -> Vec<u8> {
        self.stderr.lock().unwrap().clone()
    }

    pub fn stderr_truncated(&self) -> bool {
        self.stderr_truncated.load(std::sync::atomic::Ordering::SeqCst)
    }

    /// Force-kill the whole process tree (job terminate on Windows) and
    /// reap. Never panics; safe to call repeatedly (RT-007).
    pub fn kill(&mut self) {
        self.state = LifecycleState::Stopping;
        let _ = self.child.kill();
        let _ = self.child.wait();
        #[cfg(windows)]
        if let Some(job) = self.job.as_mut() {
            job.terminate();
        }
        self.state = LifecycleState::Stopped;
    }

    /// Graceful shutdown (review 53 §17): wait up to `shutdown_grace` for
    /// a self-exit, then force-kill the tree and reap (RT-006).
    pub fn shutdown(&mut self) -> Result<ProcessTermination, RuntimeError> {
        self.state = LifecycleState::Stopping;
        let deadline = Instant::now() + self.limits.shutdown_grace;
        while Instant::now() < deadline {
            match self.child.try_wait() {
                Ok(Some(status)) => {
                    self.finish_stop();
                    return Ok(ProcessTermination::from_exit_code(status.code()));
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => break,
            }
        }
        self.kill();
        Err(RuntimeError::ShutdownTimeout)
    }

    fn finish_stop(&mut self) {
        #[cfg(windows)]
        if let Some(job) = self.job.as_mut() {
            job.terminate();
        }
        self.state = LifecycleState::Stopped;
    }
}

impl Drop for ProcessSession {
    fn drop(&mut self) {
        // fail-safe: no orphaned process tree outlives the session
        if self.state.is_alive() {
            self.kill();
        }
    }
}
