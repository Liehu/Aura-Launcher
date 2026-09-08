//! launcher-plugin dev CLI binary (P2.4-D). All logic lives in the library
//! so in-process tests can drive `run_cli` without spawning processes.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(launcher_plugin_cli::run_cli(&args));
}
