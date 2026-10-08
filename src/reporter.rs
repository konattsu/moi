use std::collections::VecDeque;
use std::io::{IsTerminal, Write};
use std::time::Instant;

const LIVE_LINES: usize = 6;
const QUIET_FAILURE_LINES: usize = 20;

pub(crate) struct Reporter {
    writer: Box<dyn Write + Send>,
    interactive: bool,
    color: bool,
    quiet: bool,
    live_lines: VecDeque<String>,
    rendered_lines: usize,
    width: usize,
    step: Option<ProgressItem>,
    stage: Option<Stage>,
}

struct ProgressItem {
    index: usize,
    total: usize,
    name: String,
    started: Instant,
}

struct Stage {
    item: ProgressItem,
    lines_below: usize,
}

impl Reporter {
    pub(crate) fn new(settings: crate::util::tracing::StdoutSettings) -> Self {
        let stderr = std::io::stderr();
        let is_terminal = stderr.is_terminal();
        let interactive = is_terminal && settings.verbose == 0 && !settings.quiet;
        let color = is_terminal && std::env::var_os("NO_COLOR").is_none();
        let width = std::env::var("COLUMNS")
            .ok()
            .and_then(|value| value.parse().ok())
            .filter(|width| *width >= 20)
            .unwrap_or(80);
        Self {
            writer: Box::new(stderr),
            interactive,
            color,
            quiet: settings.quiet,
            live_lines: VecDeque::new(),
            rendered_lines: 0,
            width,
            step: None,
            stage: None,
        }
    }

    pub(crate) fn start_stage(&mut self, index: usize, total: usize, name: &str) {
        self.clear_live();
        if !self.quiet {
            let prefix = self.paint("1;34", "=>");
            self.line(&format!("{prefix} [{index}/{total}] {name}"));
        }
        self.stage = Some(Stage {
            item: ProgressItem {
                index,
                total,
                name: name.to_string(),
                started: Instant::now(),
            },
            lines_below: 0,
        });
    }

    pub(crate) fn finish_stage(&mut self) {
        self.clear_live();
        let Some(stage) = self.stage.take() else {
            return;
        };
        if self.interactive {
            let prefix = self.paint("1;34", "=>");
            let check = self.paint("32", "✓");
            let elapsed = format_elapsed(stage.item.started.elapsed());
            let distance = stage.lines_below + 1;
            let _ = write!(
                self.writer,
                "\x1b[{distance}A\r\x1b[2K{prefix} [{}/{}] {} {check}{elapsed}\x1b[{distance}B\r",
                stage.item.index, stage.item.total, stage.item.name,
            );
            let _ = self.writer.flush();
        }
    }

    pub(crate) fn start_step(&mut self, index: usize, total: usize, name: &str) {
        self.clear_live();
        self.live_lines.clear();
        self.step = Some(ProgressItem {
            index,
            total,
            name: name.to_string(),
            started: Instant::now(),
        });
        if !self.quiet {
            let prefix = self.paint("1;36", "==>");
            self.line(&format!("{prefix} [{index}/{total}] {name}"));
        }
    }

    pub(crate) fn finish_step(&mut self) {
        self.clear_live();
        self.live_lines.clear();
        let Some(step) = self.step.take() else {
            return;
        };
        if self.interactive {
            let prefix = self.paint("1;36", "==>");
            let check = self.paint("32", "✓");
            let elapsed = format_elapsed(step.started.elapsed());
            let _ = writeln!(
                self.writer,
                "\x1b[1A\r\x1b[2K{prefix} [{}/{}] {} {check}{elapsed}",
                step.index, step.total, step.name,
            );
            let _ = self.writer.flush();
        }
    }

    pub(crate) fn fail_step(&mut self, error: &crate::error::MoiError) {
        if self.quiet {
            let lines = self.live_lines.iter().cloned().collect::<Vec<_>>();
            for line in lines {
                self.log_line_persistent(&line);
            }
        }
        self.rendered_lines = 0;
        let elapsed = self
            .step
            .take()
            .map(|step| format_elapsed(step.started.elapsed()))
            .unwrap_or_default();
        let failed = self.paint("31", "failed");
        self.line(&format!("    {failed}: {error}{elapsed}"));
    }

    pub(crate) fn log(&mut self, message: impl AsRef<str>) {
        for line in message.as_ref().lines() {
            self.log_one(line.trim_end_matches('\r'));
        }
    }

    pub(crate) fn summary(&mut self, environment: &str, platform: &str, stages: usize) {
        if !self.quiet {
            self.line(&format!(
                "Applied {environment} ({platform}): {stages} stages"
            ));
        }
    }

    fn log_one(&mut self, line: &str) {
        if self.quiet {
            push_bounded(&mut self.live_lines, line.to_string(), QUIET_FAILURE_LINES);
            return;
        }
        if !self.interactive {
            self.log_line_persistent(line);
            return;
        }

        self.clear_live();
        push_bounded(&mut self.live_lines, line.to_string(), LIVE_LINES);
        let lines = self.live_lines.iter().cloned().collect::<Vec<_>>();
        for line in lines {
            let line = truncate(&line, self.width.saturating_sub(4));
            let line = self.paint("90", &format!("  | {line}"));
            let _ = writeln!(self.writer, "{line}");
            self.rendered_lines += 1;
        }
        let _ = self.writer.flush();
    }

    fn log_line_persistent(&mut self, line: &str) {
        let line = self.paint("90", &format!("  | {line}"));
        self.line(&line);
    }

    fn clear_live(&mut self) {
        if !self.interactive || self.rendered_lines == 0 {
            return;
        }
        for _ in 0..self.rendered_lines {
            let _ = write!(self.writer, "\x1b[1A\r\x1b[2K");
        }
        let _ = self.writer.flush();
        self.rendered_lines = 0;
    }

    fn line(&mut self, line: &str) {
        let _ = writeln!(self.writer, "{line}");
        let _ = self.writer.flush();
        if let Some(stage) = self.stage.as_mut() {
            stage.lines_below += 1;
        }
    }

    fn paint(&self, code: &str, text: &str) -> String {
        if self.color {
            format!("\x1b[{code}m{text}\x1b[0m")
        } else {
            text.to_string()
        }
    }
}

fn push_bounded(lines: &mut VecDeque<String>, line: String, capacity: usize) {
    if lines.len() == capacity {
        lines.pop_front();
    }
    lines.push_back(line);
}

fn truncate(line: &str, width: usize) -> String {
    if line.chars().count() <= width {
        return line.to_string();
    }
    let keep = width.saturating_sub(1);
    format!("{}…", line.chars().take(keep).collect::<String>())
}

fn format_elapsed(elapsed: std::time::Duration) -> String {
    if elapsed.as_secs_f64() < 1.0 {
        String::new()
    } else {
        format!(" {:.1}s", elapsed.as_secs_f64())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_lines_keeps_the_latest_entries() {
        let mut lines = VecDeque::new();
        push_bounded(&mut lines, "one".to_string(), 2);
        push_bounded(&mut lines, "two".to_string(), 2);
        push_bounded(&mut lines, "three".to_string(), 2);

        assert_eq!(lines, ["two", "three"]);
    }

    #[test]
    fn truncates_to_the_requested_character_count() {
        assert_eq!(truncate("abcdefgh", 5), "abcd…");
        assert_eq!(truncate("abc", 5), "abc");
    }

    #[test]
    fn temporary_log_style_is_gray() {
        let reporter = Reporter {
            writer: Box::new(std::io::sink()),
            interactive: true,
            color: true,
            quiet: false,
            live_lines: VecDeque::new(),
            rendered_lines: 0,
            width: 80,
            step: None,
            stage: None,
        };

        assert_eq!(
            reporter.paint("90", "  | downloading"),
            "\x1b[90m  | downloading\x1b[0m"
        );
    }
}
