//! Bounded stdio transport (review 53 §11/§12/§13): the runtime provides
//! bounded byte/line transport — NEVER protocol parsing. Line framing is
//! the deepest the runtime goes (bytes/lines are protocol-neutral); JSON,
//! NDJSON semantics, JSON-RPC and plugin framing stay with the protocol
//! owners.
//!
//! Allocation-bound enforcement (review 53 §13): a line is accumulated
//! chunk-wise up to `cap + 1` bytes — a hostile child can never make this
//! reader buffer an unbounded line. Note that `Take<BufReader>` must NOT
//! be used for this: its `fill_buf` bypasses the limit by returning the
//! whole underlying buffer.

use std::io::{BufRead, IoSliceMut};

/// Outcome of one bounded line read.
#[derive(Debug, PartialEq)]
pub enum LineOutcome {
    /// A complete line (newline stripped).
    Line(String),
    /// The line exceeded the cap (or EOF hit mid-line): a framing
    /// violation. The stream position stays in sync (the remainder of the
    /// oversized line was drained).
    LimitExceeded,
    /// Clean EOF with nothing buffered.
    Eof,
}

/// Read one `\n`-terminated line from `reader`, accumulating at most
/// `cap + 1` bytes. Oversized/truncated lines yield
/// [`LineOutcome::LimitExceeded`] with the stream left positioned after
/// the line's terminator (or EOF).
pub fn read_bounded_line<R: BufRead>(reader: &mut R, cap: usize) -> std::io::Result<LineOutcome> {
    let mut buf: Vec<u8> = Vec::with_capacity(4096.min(cap + 1));
    let mut oversized = false;
    loop {
        enum Action {
            EmitLine(usize),
            LimitAndSkip(usize),
            Consume(usize),
            Eof,
        }
        let action: Action = {
            let available = reader.fill_buf()?;
            if available.is_empty() {
                if oversized || !buf.is_empty() {
                    // truncated line at EOF: framing violation
                    Action::LimitAndSkip(0)
                } else {
                    Action::Eof
                }
            } else if let Some(pos) = available.iter().position(|&b| b == b'\n') {
                if oversized || pos + 1 > cap {
                    Action::LimitAndSkip(pos + 1)
                } else {
                    buf.extend_from_slice(&available[..pos]);
                    Action::EmitLine(pos + 1)
                }
            } else {
                let take = available.len().min(cap.saturating_sub(buf.len()) + 1);
                buf.extend_from_slice(&available[..take]);
                if buf.len() > cap {
                    oversized = true;
                    buf.clear();
                }
                Action::Consume(available.len())
            }
        };
        match action {
            Action::Eof => return Ok(LineOutcome::Eof),
            Action::EmitLine(n) => {
                // buf already excludes the newline (copied [..pos])
                reader.consume(n);
                return Ok(LineOutcome::Line(String::from_utf8_lossy(&buf).into_owned()));
            }
            Action::LimitAndSkip(n) => {
                if n > 0 {
                    // drain the remainder of the oversized line so the next
                    // read stays in sync
                    let mut skipped = 0;
                    while skipped < n {
                        let available = reader.fill_buf()?;
                        if available.is_empty() {
                            break;
                        }
                        let take = available.len().min(n - skipped);
                        skipped += take;
                        reader.consume(take);
                    }
                }
                return Ok(LineOutcome::LimitExceeded);
            }
            Action::Consume(n) => reader.consume(n),
        }
    }
}

// keep the multi-read import honest for future chunk APIs
#[allow(dead_code)]
fn _io_slice_marker(_: Option<IoSliceMut<'_>>) {}
