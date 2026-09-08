use std::io::Write;

// contract "stdout red line": writes diagnostic text to stdout, violating
// the "stdout is protocol frames only" invariant -> host must reject.
fn main() {
    let mut out = std::io::stdout();
    let _ = out.write_all(b"debug: starting plugin\n");
    let _ = out.flush();
    // then serve nothing; the host must already have failed the handshake
    std::thread::sleep(std::time::Duration::from_secs(30));
}
