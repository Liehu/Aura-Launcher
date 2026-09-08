//! Adversarial child fixture for launcher-runtime tests. Behavior selected
//! by argv[1]; speaks nothing protocol-specific — pure process/IO mechanics.

use std::io::{BufRead, Write};
use std::process::exit;

fn main() {
    let mode = std::env::args().nth(1).unwrap_or_default();
    match mode.as_str() {
        // exit immediately with a code
        "exit" => exit(std::env::args().nth(2).and_then(|c| c.parse().ok()).unwrap_or(0)),
        // hang forever
        "hang" => loop {
            std::thread::sleep(std::time::Duration::from_secs(3600));
        },
        // spawn a grandchild that hangs, then hang (process-tree test)
        "tree" => {
            let me = std::env::current_exe().unwrap();
            let _ = std::process::Command::new(me)
                .args(["hang"])
                .spawn()
                .expect("grandchild spawns");
            loop {
                std::thread::sleep(std::time::Duration::from_secs(3600));
            }
        }
        // line protocol: echo lines back, with special modes
        _ => {
            let stdin = std::io::stdin();
            let mut out = std::io::stdout();
            eprintln!("rt-child diagnostics start"); // stderr diagnostics
            for line in stdin.lock().lines() {
                let Ok(line) = line else { break };
                if let Some(arg) = line.strip_prefix("echo ") {
                    let _ = writeln!(out, "{arg}");
                    let _ = out.flush();
                } else if line == "big" {
                    // oversized single line (default cap 256 KiB): 300 KiB
                    let _ = writeln!(out, "{}", "x".repeat(300 * 1024));
                    let _ = out.flush();
                } else if line == "dribble" {
                    // partial write: half a line, pause, rest (RT-013)
                    let _ = write!(out, "hal");
                    let _ = out.flush();
                    std::thread::sleep(std::time::Duration::from_millis(150));
                    let _ = writeln!(out, "f-line");
                    let _ = out.flush();
                } else if line == "crash" {
                    exit(3);
                } else if line == "quit" {
                    exit(0);
                } else {
                    let _ = writeln!(out, "ERR unknown command");
                    let _ = out.flush();
                }
            }
        }
    }
}
