//! Writer adapter that translates `\n` → `\r\n` for raw-mode terminals.

use std::io::{self, Write};

/// Wraps any `Write` and converts bare `\n` to `\r\n`.
///
/// Also tracks whether the last byte written was a newline so the REPL can
/// detect dangling output (e.g. `>> x` without `¶`) and add a line break
/// before the next prompt.
pub struct RawModeWriter<W: Write> {
    inner: W,
    /// True when the last byte written (after translation) ended with `\n`,
    /// or when no output has been produced yet (clean-slate assumption).
    last_was_newline: bool,
}

impl<W: Write> RawModeWriter<W> {
    pub fn new(inner: W) -> Self {
        Self { inner, last_was_newline: true }
    }

    /// Whether the last byte written to this writer was a newline.
    /// Returns `true` if no output has been produced since the last reset.
    pub fn ended_with_newline(&self) -> bool {
        self.last_was_newline
    }

    /// Reset the newline-tracking flag to `true` (clean slate).
    /// Call this at the start of each REPL execution cycle.
    pub fn reset_newline_tracking(&mut self) {
        self.last_was_newline = true;
    }
}

impl<W: Write> Write for RawModeWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }

        self.last_was_newline = buf.last() == Some(&b'\n');

        let mut start = 0;
        for (i, &b) in buf.iter().enumerate() {
            if b == b'\n' {
                if i > start {
                    self.inner.write_all(&buf[start..i])?;
                }
                self.inner.write_all(b"\r\n")?;
                start = i + 1;
            }
        }
        if start < buf.len() {
            self.inner.write_all(&buf[start..])?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}
