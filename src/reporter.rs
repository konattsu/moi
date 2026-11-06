const LIVE_LINES: usize = 6;

pub(crate) struct Reporter {
    writer: Box<dyn std::io::Write + Send>,
    quiet: bool,
    interactive: bool,
    color: bool,
    width: usize,
    stage: Option<ProgressItem>,
    step: Option<ProgressItem>,
    live: std::collections::VecDeque<String>,
    rendered_lines: usize,
    output: Option<std::fs::File>,
    recent: std::collections::VecDeque<Vec<u8>>,
}

struct ProgressItem {
    label: String,
    started: std::time::Instant,
}

impl Reporter {
    pub(crate) fn new(settings: crate::util::tracing::StdoutSettings) -> Self {
        use std::io::IsTerminal;
        let terminal = std::io::stderr().is_terminal();
        Self {
            writer: Box::new(std::io::stderr()),
            quiet: settings.quiet,
            interactive: terminal && settings.verbose == 0 && !settings.quiet,
            color: terminal && std::env::var_os("NO_COLOR").is_none(),
            width: std::env::var("COLUMNS")
                .ok()
                .and_then(|value| value.parse().ok())
                .filter(|width| *width >= 20)
                .unwrap_or(80),
            stage: None,
            step: None,
            live: std::collections::VecDeque::new(),
            rendered_lines: 0,
            output: None,
            recent: std::collections::VecDeque::new(),
        }
    }

    pub(crate) fn start_stage(&mut self, index: usize, total: usize, name: &str) {
        self.begin();
        let label = format!("[{index}/{total}] {name}");
        if !self.quiet {
            self.line(&format!("=> {label}"));
        }
        self.stage = Some(ProgressItem {
            label,
            started: std::time::Instant::now(),
        });
    }

    pub(crate) fn finish_stage(&mut self, outcome: &str) {
        self.clear_live();
        if let Some(stage) = self.stage.take()
            && !self.quiet
        {
            self.line(&format!(
                "=> {}: {outcome} ({:.1}s)",
                stage.label,
                stage.started.elapsed().as_secs_f64()
            ));
        }
        self.output = None;
        self.recent.clear();
    }

    pub(crate) fn start_step(&mut self, index: usize, total: usize, name: &str) {
        self.begin();
        let label = format!("[{index}/{total}] {name}");
        if !self.quiet {
            self.line(&format!("  ==> {label}"));
        }
        self.step = Some(ProgressItem {
            label,
            started: std::time::Instant::now(),
        });
    }

    pub(crate) fn finish_step(&mut self, outcome: &str) {
        use std::io::Write;
        self.clear_live();
        if let Some(step) = self.step.take()
            && !self.quiet
        {
            if self.interactive {
                let _ = write!(self.writer, "\x1b[1A\r\x1b[2K");
            }
            self.line(&format!(
                "  ==> {}: {outcome} ({:.1}s)",
                step.label,
                step.started.elapsed().as_secs_f64()
            ));
        }
        self.live.clear();
        if outcome != "failed" {
            self.output = None;
            self.recent.clear();
        }
    }

    pub(crate) fn log(&mut self, message: impl std::fmt::Display) {
        self.child_output(format!("{message}\n").as_bytes());
    }

    pub(crate) fn child_output(&mut self, bytes: &[u8]) {
        use std::io::Write;
        if bytes.is_empty() {
            return;
        }
        if self.interactive || self.quiet {
            if let Some(output) = &mut self.output
                && output.write_all(bytes).is_err()
            {
                self.output = None;
            }
            self.recent.push_back(bytes.to_vec());
            if self.recent.len() > 50 {
                self.recent.pop_front();
            }
        }
        if self.quiet {
            return;
        }
        if !self.interactive || self.step.is_none() {
            let _ = self.writer.write_all(bytes);
            let _ = self.writer.flush();
            return;
        }
        self.clear_live();
        for line in String::from_utf8_lossy(bytes).lines() {
            self.live.push_back(line.to_string());
            if self.live.len() > LIVE_LINES {
                self.live.pop_front();
            }
        }
        for index in 0..LIVE_LINES {
            let text = self.live.get(index).map(String::as_str).unwrap_or("");
            let text = fit_line(&format!("    | {text}"), self.width.saturating_sub(1));
            if self.color {
                let _ = writeln!(self.writer, "\x1b[90m{text}\x1b[0m");
            } else {
                let _ = writeln!(self.writer, "{text}");
            }
        }
        self.rendered_lines = LIVE_LINES;
        let _ = self.writer.flush();
    }

    fn begin(&mut self) {
        self.clear_live();
        self.live.clear();
        self.recent.clear();
        self.output = if self.interactive || self.quiet {
            tempfile::tempfile().ok()
        } else {
            None
        };
    }

    pub(crate) fn failure(&mut self) {
        use std::io::{Seek, Write};
        let replay = self.quiet || (self.interactive && self.step.is_some());
        self.finish_step("failed");
        if replay {
            let restored = self.output.take().is_some_and(|mut output| {
                output.rewind().is_ok()
                    && std::io::copy(&mut output, &mut self.writer).is_ok()
            });
            if !restored {
                for bytes in self.recent.drain(..) {
                    let _ = self.writer.write_all(&bytes);
                }
            }
            let _ = self.writer.write_all(b"\n");
            let _ = self.writer.flush();
        }
        self.finish_stage("failed");
    }

    fn clear_live(&mut self) {
        use std::io::Write;
        for _ in 0..self.rendered_lines {
            let _ = write!(self.writer, "\x1b[1A\r\x1b[2K");
        }
        self.rendered_lines = 0;
        let _ = self.writer.flush();
    }

    fn line(&mut self, text: &str) {
        use std::io::Write;
        let text = if self.interactive {
            fit_line(text, self.width.saturating_sub(1))
        } else {
            text.to_string()
        };
        let _ = writeln!(self.writer, "{text}");
        let _ = self.writer.flush();
    }
}

// Use a conservative two-column width for non-ASCII characters so live output
// cannot wrap and leave stale lines behind. Control sequences never reach the UI.
fn fit_line(text: &str, width: usize) -> String {
    let mut remaining = width;
    text.chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take_while(|c| {
            let columns = if c.is_ascii() { 1 } else { 2 };
            if remaining < columns {
                return false;
            }
            remaining -= columns;
            true
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Default)]
    struct Capture(std::sync::Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for Capture {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    impl Capture {
        fn take(&self) -> String {
            String::from_utf8(std::mem::take(&mut *self.0.lock().unwrap())).unwrap()
        }
    }

    fn reporter(interactive: bool, quiet: bool) -> (Reporter, Capture) {
        let capture = Capture::default();
        let mut reporter =
            Reporter::new(crate::util::tracing::StdoutSettings { quiet, verbose: 0 });
        reporter.writer = Box::new(capture.clone());
        reporter.interactive = interactive;
        reporter.color = interactive;
        reporter.width = 40;
        (reporter, capture)
    }

    #[test]
    fn live_output_uses_six_gray_lines_and_is_removed_on_success() {
        let (mut reporter, capture) = reporter(true, false);
        reporter.start_stage(3, 3, "apply modules");
        reporter.start_step(1, 1, "shell");
        capture.take();
        reporter.child_output(b"one\n");
        let first = capture.take();
        assert_eq!(first.matches("\x1b[90m").count(), LIVE_LINES);
        reporter.child_output(b"two\nthree\nfour\nfive\nsix\nseven\neight\n");
        let next = capture.take();
        assert_eq!(next.matches("\x1b[1A\r\x1b[2K").count(), LIVE_LINES);
        assert_eq!(next.matches("\x1b[90m").count(), LIVE_LINES);
        assert!(!next.contains("| one"));
        assert!(!next.contains("| two"));
        assert!(next.contains("| eight"));
        reporter.finish_step("executed");
        let finished = capture.take();
        assert_eq!(finished.matches("\x1b[1A\r\x1b[2K").count(), LIVE_LINES + 1);
        assert!(finished.contains(": executed ("));
        assert!(!finished.contains("| eight"));
        assert!(reporter.output.is_none());
        assert!(reporter.recent.is_empty());
    }

    #[test]
    fn failure_replays_all_output_without_live_window_truncation() {
        for (interactive, quiet) in [(true, false), (false, true)] {
            let (mut reporter, capture) = reporter(interactive, quiet);
            reporter.start_stage(3, 3, "apply modules");
            reporter.start_step(1, 1, "shell");
            let first = format!("first error: {}\n", "long details ".repeat(20));
            reporter.child_output(first.as_bytes());
            for index in 0..100 {
                reporter.child_output(format!("output {index}\n").as_bytes());
            }
            capture.take();
            reporter.failure();
            let failure = capture.take();
            assert!(failure.contains(&first));
            assert!(failure.contains("output 0\n"));
            assert!(failure.contains("output 99\n"));
            if quiet {
                assert!(!failure.contains("==>"));
                assert!(!failure.contains('\x1b'));
            } else {
                assert!(failure.contains(": failed ("));
            }
        }
    }

    #[test]
    fn redirected_output_is_complete_and_never_erased_or_replayed() {
        let (mut reporter, capture) = reporter(false, false);
        reporter.start_step(1, 1, "shell");
        capture.take();
        let bytes = format!("{}\nsecond line\n", "long details ".repeat(20));
        reporter.child_output(bytes.as_bytes());
        assert_eq!(capture.take(), bytes);
        reporter.failure();
        let failure = capture.take();
        assert!(!failure.contains("long details"));
        assert!(!failure.contains('\x1b'));
    }

    #[test]
    fn failed_step_does_not_replay_output_from_a_successful_step() {
        let (mut reporter, capture) = reporter(true, false);
        reporter.start_step(1, 2, "first");
        reporter.child_output(b"successful output\n");
        reporter.finish_step("done");
        reporter.start_step(2, 2, "second");
        reporter.child_output(b"failed output\n");
        capture.take();
        reporter.failure();
        let failure = capture.take();
        assert!(failure.contains("failed output"));
        assert!(!failure.contains("successful output"));
    }

    #[test]
    fn live_lines_fit_the_terminal_and_neutralize_control_characters() {
        assert_eq!(fit_line("abcdef", 4), "abcd");
        assert_eq!(fit_line("あいうえ", 5), "あい");
        assert_eq!(fit_line("a\r\nb\x1b", 20), "a  b ");
    }
}
