pub(crate) struct Reporter {
    quiet: bool,
    recent: std::collections::VecDeque<Vec<u8>>,
}
impl Reporter {
    pub(crate) fn new(settings: crate::util::tracing::StdoutSettings) -> Self {
        Self {
            quiet: settings.quiet,
            recent: std::collections::VecDeque::new(),
        }
    }
    pub(crate) fn log(&mut self, message: impl std::fmt::Display) {
        if !self.quiet {
            eprintln!("{message}");
        }
    }
    pub(crate) fn child_output(&mut self, bytes: &[u8]) {
        use std::io::Write;
        if !self.quiet {
            let _ = std::io::stderr().lock().write_all(bytes);
        }
        self.recent.push_back(bytes.to_vec());
        if self.recent.len() > 50 {
            self.recent.pop_front();
        }
    }
    pub(crate) fn begin(&mut self) {
        self.recent.clear();
    }
    pub(crate) fn failure(&mut self) {
        use std::io::Write;
        if self.quiet {
            for bytes in self.recent.drain(..) {
                let _ = std::io::stderr().lock().write_all(&bytes);
            }
        }
    }
}
