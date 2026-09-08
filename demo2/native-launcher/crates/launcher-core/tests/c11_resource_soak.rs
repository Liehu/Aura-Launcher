//! P2.3-C11 Long-Run Resource Soak (scaled profile): drives the production
//! Core through a continuous search/history/favorite/favorite-toggle churn
//! while sampling self RSS, handle count and thread count. Growth must stay
//! BOUNDED — a leak shows up as monotonic drift regardless of duration.
//!
//! Duration profiles (LAUNCHER_SOAK_SECONDS):
//!   default 6  — CI/test-suite profile (still exercises thousands of cycles)
//!   1800       — 30-minute profile
//!   14400      — 4-hour profile (manual/nightly)
//! The assertion is on BOUNDED growth between the warmup sample and the
//! final sample, not on duration, so every profile shares one contract.

use std::time::{Duration, Instant};

use launcher_core::{favorites::FavoriteService, Core, Provider};
use launcher_domain::{Action, ActionKind, Category, Command, QueryContext};
use launcher_indexer::Indexer;

#[cfg(windows)]
mod win {
    use windows::Win32::System::Diagnostics::ToolHelp::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use windows::Win32::System::ProcessStatus::GetProcessMemoryInfo;
    use windows::Win32::System::Threading::{
        GetCurrentProcess, GetCurrentProcessId, GetProcessHandleCount,
    };

    pub struct Sample {
        pub rss_mb: f64,
        pub handles: u32,
        pub threads: u32,
    }

    pub fn sample() -> Sample {
        unsafe {
            let mut pmc: windows::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS =
                std::mem::zeroed();
            pmc.cb =
                std::mem::size_of::<windows::Win32::System::ProcessStatus::PROCESS_MEMORY_COUNTERS>()
                    as u32;
            let rss_mb = if GetProcessMemoryInfo(GetCurrentProcess(), &mut pmc, pmc.cb).is_ok() {
                pmc.WorkingSetSize as f64 / (1024.0 * 1024.0)
            } else {
                0.0
            };
            let mut handles = 0u32;
            let _ = GetProcessHandleCount(GetCurrentProcess(), &mut handles);
            let threads = thread_count(GetCurrentProcessId());
            Sample { rss_mb, handles, threads }
        }
    }

    fn thread_count(pid: u32) -> u32 {
        unsafe {
            let snap = match CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) {
                Ok(h) => h,
                Err(_) => return 0,
            };
            let mut entry: PROCESSENTRY32W = std::mem::zeroed();
            entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as u32;
            let mut threads = 0u32;
            if Process32FirstW(snap, &mut entry).is_ok() {
                loop {
                    if entry.th32ProcessID == pid {
                        threads = entry.cntThreads;
                        break;
                    }
                    if Process32NextW(snap, &mut entry).is_err() {
                        break;
                    }
                }
            }
            threads
        }
    }
}

struct MockProvider {
    id: String,
}

impl Provider for MockProvider {
    fn id(&self) -> &str {
        &self.id
    }
    fn query(&mut self, q: &QueryContext) -> Vec<Command> {
        let needle = q.normalized.clone();
        (0..8)
            .map(|i| Command {
                id: format!("{needle}-{i}"),
                title: format!("{} Mock App {i}", capitalize(&needle)),
                subtitle: None,
                icon: None,
                provider_id: self.id.clone(),
                score: 0.0,
                keywords: vec![],
                category: Category::Application,
                actions: vec![Action {
                    kind: ActionKind::Open,
                    payload: None,
                    id: None,
                    title: None,
                    disabled_reason: None,
                    shortcut: None,
                    confirmation_required: false,
                }],
                target: None,
            })
            .collect()
    }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[test]
fn long_run_resource_soak_growth_is_bounded() {
    let soak_secs: u64 = std::env::var("LAUNCHER_SOAK_SECONDS")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(6);

    // real persistence layers under the churn
    let dir = std::env::temp_dir().join(format!("nl_c11_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();

    let mut core = Core::new();
    let mut index = Indexer::open(&dir.join("idx.db")).unwrap();
    index.rebuild(&[dir.clone()]).unwrap();
    core.set_history(index);
    core.set_favorites(FavoriteService::open(&dir.join("fav.db")).unwrap());
    for p in ["p1", "p2", "p3"] {
        core.register(Box::new(MockProvider { id: p.into() }));
    }

    // warmup: allocations for caches/connections settle before the baseline
    run_cycles(&mut core, 200);
    #[cfg(windows)]
    let start = win::sample();
    #[cfg(not(windows))]
    let start = ();

    let deadline = Instant::now() + Duration::from_secs(soak_secs);
    let mut cycles = 0u64;
    while Instant::now() < deadline {
        run_cycles(&mut core, 500);
        cycles += 500;
    }

    #[cfg(windows)]
    let end = win::sample();
    #[cfg(not(windows))]
    let end = ();

    #[cfg(windows)]
    {
        let rss_growth = end.rss_mb - start.rss_mb;
        // bounded, not zero: caches are LRU-bounded, WAL checkpoints keep
        // the DB file in check; drift beyond this indicates a leak
        assert!(
            rss_growth < 60.0,
            "RSS leaked: +{rss_growth:.1}MB over {cycles} cycles \
             (baseline {:.1}MB → final {:.1}MB)",
            start.rss_mb,
            end.rss_mb
        );
        assert!(
            end.threads <= start.threads + 2,
            "thread leak: {} → {}",
            start.threads,
            end.threads
        );
        assert!(
            end.handles <= start.handles + 64,
            "handle leak: {} → {}",
            start.handles,
            end.handles
        );
        println!(
            "soak: {cycles} cycles in {soak_secs}s | RSS {:.1}→{:.1}MB (+{rss_growth:.1}) | threads {}→{} | handles {}→{}",
            start.rss_mb, end.rss_mb, start.threads, end.threads, start.handles, end.handles
        );
    }
    #[cfg(not(windows))]
    {
        let _ = (start, end);
        println!("soak: {cycles} cycles in {soak_secs}s (no resource sampling on this platform)");
    }

    core.search("final", 10); // still healthy at the end
    std::fs::remove_dir_all(&dir).ok();
}

fn run_cycles(core: &mut Core, n: u64) {
    for i in 0..n {
        let q = format!("query {}", i % 37); // bounded query set → cache churn + reuse
        let r = core.search(&q, 10);
        if let Some(c) = r.commands.first() {
            if i % 3 == 0 {
                core.record_use_with_title(&c.id, &c.provider_id, &c.title);
            }
            if i % 11 == 0 {
                let _ = core.toggle_favorite(c);
            }
        }
    }
}
