use std::io::{self, IsTerminal, Write};

/// Environment and stream facts that determine whether live terminal progress
/// is shown. Tests construct this directly instead of mutating the process
/// environment so parallel tests stay isolated.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerminalFacts {
    pub stderr_is_terminal: bool,
    pub term: Option<String>,
    pub ci: Option<String>,
    pub github_actions: Option<String>,
    pub gitlab_ci: Option<String>,
    pub buildkite: Option<String>,
    pub tf_build: Option<String>,
    pub jenkins_url: Option<String>,
    pub no_color: Option<String>,
    pub columns: Option<String>,
    /// Width queried from the stderr terminal itself. When present it takes
    /// precedence over the `COLUMNS` hint, which environments can export stale
    /// or incorrect values for.
    pub terminal_width: Option<usize>,
}

impl TerminalFacts {
    pub fn from_environment() -> Self {
        Self {
            stderr_is_terminal: io::stderr().is_terminal(),
            term: std::env::var("TERM").ok(),
            ci: std::env::var("CI").ok(),
            github_actions: std::env::var("GITHUB_ACTIONS").ok(),
            gitlab_ci: std::env::var("GITLAB_CI").ok(),
            buildkite: std::env::var("BUILDKITE").ok(),
            tf_build: std::env::var("TF_BUILD").ok(),
            jenkins_url: std::env::var("JENKINS_URL").ok(),
            no_color: std::env::var("NO_COLOR").ok(),
            columns: std::env::var("COLUMNS").ok(),
            terminal_width: stderr_terminal_width(),
        }
    }

    /// A non-empty `CI` value is active unless it is `0` or `false`
    /// (case-insensitive). The provider markers always mark CI when non-empty,
    /// even when `CI` itself is falsy.
    pub fn ci_active(&self) -> bool {
        let direct = self.ci.as_deref().is_some_and(ci_value_active);
        let provider = [
            &self.github_actions,
            &self.gitlab_ci,
            &self.buildkite,
            &self.tf_build,
            &self.jenkins_url,
        ]
        .into_iter()
        .any(|value| value.as_deref().is_some_and(|value| !value.is_empty()));
        direct || provider
    }

    pub fn progress_eligible(&self, json_mode: bool) -> bool {
        self.stderr_is_terminal
            && self.term.as_deref() != Some("dumb")
            && !self.ci_active()
            && !json_mode
    }

    /// `NO_COLOR` suppresses styling only; it never suppresses progress.
    pub fn progress_styled(&self) -> bool {
        !self
            .no_color
            .as_deref()
            .is_some_and(|value| !value.is_empty())
    }

    /// Best-effort terminal width. The queried stderr terminal width wins over
    /// the `COLUMNS` environment hint. When both are unavailable a conservative
    /// default keeps lines short so replacements never wrap.
    pub fn width(&self) -> usize {
        self.terminal_width
            .or_else(|| {
                self.columns
                    .as_deref()
                    .and_then(|value| value.trim().parse::<usize>().ok())
                    .filter(|width| *width > 0)
            })
            .unwrap_or(80)
    }
}

/// Query the stderr terminal's current width through the platform adapter.
/// Any failure or unsupported platform returns `None` so callers can fall back
/// conservatively instead of trusting `COLUMNS` blindly.
#[cfg(unix)]
fn stderr_terminal_width() -> Option<usize> {
    use std::os::unix::io::AsRawFd;

    #[repr(C)]
    #[derive(Default)]
    struct WindowSize {
        rows: u16,
        columns: u16,
        x_pixels: u16,
        y_pixels: u16,
    }

    #[cfg(any(target_os = "linux", target_os = "android"))]
    const TIOCGWINSZ: std::ffi::c_ulong = 0x5413;
    // BSD-family terminals (including macOS) encode the request differently.
    #[cfg(not(any(target_os = "linux", target_os = "android")))]
    const TIOCGWINSZ: std::ffi::c_ulong = 0x40087468;

    unsafe extern "C" {
        fn ioctl(fd: std::ffi::c_int, request: std::ffi::c_ulong, ...) -> std::ffi::c_int;
    }

    let mut size = WindowSize::default();
    // SAFETY: `ioctl` receives the valid stderr descriptor and a pointer to a
    // correctly sized `winsize` buffer for `TIOCGWINSZ`. A query against a
    // non-terminal fails with `ENOTTY` and is handled as unavailable.
    let result = unsafe { ioctl(std::io::stderr().as_raw_fd(), TIOCGWINSZ, &mut size) };
    (result == 0 && size.columns > 0).then_some(size.columns as usize)
}

#[cfg(not(unix))]
fn stderr_terminal_width() -> Option<usize> {
    None
}

fn ci_value_active(value: &str) -> bool {
    !value.is_empty() && value != "0" && !value.eq_ignore_ascii_case("false")
}

/// Owns the single transient status line. Only one instance writes progress;
/// updates are serialized by construction and only changed content is written.
pub struct TransientLine<W: Write> {
    writer: W,
    enabled: bool,
    width: usize,
    styled: bool,
    displayed: bool,
    last: String,
}

impl<W: Write> TransientLine<W> {
    pub fn new(writer: W, enabled: bool, width: usize, styled: bool) -> Self {
        Self {
            writer,
            enabled,
            width: width.max(1),
            styled,
            displayed: false,
            last: String::new(),
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Render `text`, replacing the previous transient line. Unchanged content
    /// and disabled rendering produce no bytes. A failing writer disables
    /// rendering without propagating the error to callers.
    pub fn update(&mut self, text: &str) {
        if !self.enabled {
            return;
        }
        let clamped = clamp_to_width(text, self.width);
        if self.displayed && clamped == self.last {
            return;
        }
        if self.render(&clamped).is_err() {
            self.enabled = false;
            self.displayed = false;
            return;
        }
        self.last = clamped;
        self.displayed = true;
    }

    /// Erase the transient line before final reporting. Cleanup is not a metric
    /// event and never re-emits the last snapshot.
    pub fn clear(&mut self) {
        if !self.enabled || !self.displayed {
            return;
        }
        if self
            .writer
            .write_all(b"\r\x1b[K")
            .and_then(|()| self.writer.flush())
            .is_err()
        {
            self.enabled = false;
        }
        self.displayed = false;
    }

    fn render(&mut self, text: &str) -> io::Result<()> {
        self.writer.write_all(b"\r\x1b[K")?;
        if self.styled && !text.is_empty() {
            write!(self.writer, "\x1b[1m{text}\x1b[0m")?;
        } else {
            self.writer.write_all(text.as_bytes())?;
        }
        self.writer.flush()
    }
}

fn clamp_to_width(text: &str, width: usize) -> String {
    let width = width.max(1);
    if text.chars().count() <= width {
        return text.to_owned();
    }
    text.chars().take(width).collect()
}

#[cfg(test)]
mod tests {
    use super::{TerminalFacts, TransientLine};
    use bloomery_test_macros::bloomery;
    use std::io::{self, Write};
    use std::sync::{Arc, Mutex};

    fn eligible() -> TerminalFacts {
        TerminalFacts {
            stderr_is_terminal: true,
            ..TerminalFacts::default()
        }
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-001")]
    fn eligible_user_terminals_render_progress() {
        let facts = eligible();
        assert!(facts.progress_eligible(false));
        let mut output = Vec::new();
        let mut line = TransientLine::new(&mut output, true, 80, false);
        line.update("checks 0/2 complete");
        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.contains("checks 0/2 complete"));
        assert!(text.starts_with('\r'));
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-002")]
    fn redirected_stderr_never_writes_progress() {
        let facts = TerminalFacts::default();
        assert!(!facts.progress_eligible(false));
        let mut output = Vec::new();
        let mut line = TransientLine::new(&mut output, false, 80, false);
        line.update("checks 1/2 complete");
        line.clear();
        assert!(output.is_empty());
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-003")]
    fn ci_markers_suppress_progress_even_on_a_terminal() {
        let active = TerminalFacts {
            stderr_is_terminal: true,
            ci: Some("1".to_owned()),
            ..TerminalFacts::default()
        };
        assert!(active.ci_active());
        assert!(!active.progress_eligible(false));

        for value in ["", "0", "false", "FALSE"] {
            let inactive = TerminalFacts {
                stderr_is_terminal: true,
                ci: Some(value.to_owned()),
                ..TerminalFacts::default()
            };
            assert!(!inactive.ci_active(), "CI={value:?} should be inactive");
            assert!(inactive.progress_eligible(false));
        }

        for (field, facts) in [
            (
                "GITHUB_ACTIONS",
                TerminalFacts {
                    stderr_is_terminal: true,
                    ci: Some("false".to_owned()),
                    github_actions: Some("true".to_owned()),
                    ..TerminalFacts::default()
                },
            ),
            (
                "GITLAB_CI",
                TerminalFacts {
                    stderr_is_terminal: true,
                    gitlab_ci: Some("true".to_owned()),
                    ..TerminalFacts::default()
                },
            ),
            (
                "BUILDKITE",
                TerminalFacts {
                    stderr_is_terminal: true,
                    buildkite: Some("true".to_owned()),
                    ..TerminalFacts::default()
                },
            ),
            (
                "TF_BUILD",
                TerminalFacts {
                    stderr_is_terminal: true,
                    tf_build: Some("True".to_owned()),
                    ..TerminalFacts::default()
                },
            ),
            (
                "JENKINS_URL",
                TerminalFacts {
                    stderr_is_terminal: true,
                    jenkins_url: Some("https://ci.example".to_owned()),
                    ..TerminalFacts::default()
                },
            ),
        ] {
            assert!(facts.ci_active(), "{field} should mark CI");
            assert!(!facts.progress_eligible(false));
        }
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-004")]
    fn json_mode_suppresses_progress() {
        assert!(!eligible().progress_eligible(true));
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-005")]
    fn dumb_terminals_suppress_progress() {
        let facts = TerminalFacts {
            stderr_is_terminal: true,
            term: Some("dumb".to_owned()),
            ..TerminalFacts::default()
        };
        assert!(!facts.progress_eligible(false));
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-006")]
    fn no_color_keeps_progress_without_styling() {
        let facts = TerminalFacts {
            stderr_is_terminal: true,
            no_color: Some("1".to_owned()),
            ..TerminalFacts::default()
        };
        assert!(facts.progress_eligible(false));
        assert!(!facts.progress_styled());

        let mut plain = Vec::new();
        let mut line = TransientLine::new(&mut plain, true, 80, facts.progress_styled());
        line.update("checks 1/2 complete");
        let plain = String::from_utf8(plain).expect("UTF-8");
        assert!(plain.contains("checks 1/2 complete"));
        assert!(!plain.contains("\x1b[1m"));

        let mut styled = Vec::new();
        let mut line = TransientLine::new(&mut styled, true, 80, true);
        line.update("checks 1/2 complete");
        let styled = String::from_utf8(styled).expect("UTF-8");
        assert!(styled.contains("\x1b[1mchecks 1/2 complete\x1b[0m"));
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-024")]
    fn changed_snapshots_trigger_redraws() {
        let mut output = Vec::new();
        let mut line = TransientLine::new(&mut output, true, 80, false);
        line.update("checks 0/2 complete");
        line.update("checks 1/2 complete");
        let text = String::from_utf8(output).expect("UTF-8");
        assert!(text.contains("checks 0/2 complete"));
        assert!(text.contains("checks 1/2 complete"));
        assert_eq!(text.matches("checks").count(), 2);
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-025")]
    fn unchanged_snapshots_produce_no_writes() {
        let mut output = Vec::new();
        {
            let mut line = TransientLine::new(&mut output, true, 80, false);
            line.update("checks 1/2 complete");
            line.update("checks 1/2 complete");
            line.update("checks 1/2 complete");
        }
        assert_eq!(
            String::from_utf8(output).expect("UTF-8"),
            "\r\u{1b}[Kchecks 1/2 complete"
        );
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-026")]
    fn updates_replace_one_line_and_never_wrap_when_narrow() {
        let mut output = Vec::new();
        {
            let mut line = TransientLine::new(&mut output, true, 10, false);
            line.update("checks 0/2 complete \u{b7} built 8");
            line.update("checks 1/2 complete \u{b7} built 9");
        }
        let text = String::from_utf8(output).expect("UTF-8");
        assert!(!text.contains('\n'));
        let lines = text
            .split("\r\u{1b}[K")
            .filter(|segment| !segment.is_empty())
            .collect::<Vec<_>>();
        assert_eq!(lines.len(), 2);
        assert!(lines.iter().all(|segment| segment.chars().count() <= 10));
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-027")]
    fn concurrent_updates_share_one_serialized_writer() {
        #[derive(Clone)]
        struct Shared(Arc<Mutex<Vec<u8>>>);
        impl Write for Shared {
            fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
                self.0
                    .lock()
                    .expect("shared writer")
                    .extend_from_slice(buffer);
                Ok(buffer.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }

        let buffer = Arc::new(Mutex::new(Vec::new()));
        let line = Arc::new(Mutex::new(TransientLine::new(
            Shared(buffer.clone()),
            true,
            80,
            false,
        )));
        let mut handles = Vec::new();
        for index in 0..8 {
            let line = line.clone();
            handles.push(std::thread::spawn(move || {
                let mut line = line.lock().expect("progress lock");
                line.update(&format!("checks {index}/8 complete"));
            }));
        }
        for handle in handles {
            handle.join().expect("progress worker");
        }
        let bytes = buffer.lock().expect("shared writer").clone();
        let text = String::from_utf8(bytes).expect("UTF-8");
        // Each redraw is exactly one replace prefix plus one snapshot fragment;
        // no snapshot can be interleaved inside another.
        for segment in text.split("\r\u{1b}[K").filter(|s| !s.is_empty()) {
            assert!(
                segment.starts_with("checks "),
                "interleaved write: {segment:?}"
            );
            assert_eq!(segment.matches("complete").count(), 1);
        }
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-028")]
    fn final_reporting_clears_the_transient_line() {
        let mut output = Vec::new();
        {
            let mut line = TransientLine::new(&mut output, true, 80, false);
            line.update("checks 1/2 complete");
            line.clear();
            // Clearing twice or clearing an empty line writes nothing more.
            line.clear();
        }
        let text = String::from_utf8(output).expect("UTF-8");
        assert_eq!(text, "\r\u{1b}[Kchecks 1/2 complete\r\u{1b}[K");
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-029")]
    fn a_failing_progress_sink_disables_rendering() {
        struct Failing;
        impl Write for Failing {
            fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed"))
            }
        }

        let mut line = TransientLine::new(Failing, true, 80, false);
        line.update("checks 1/2 complete");
        assert!(!line.is_enabled());
        // Further updates are silently ignored rather than panicking.
        line.update("checks 2/2 complete");
        line.clear();
    }

    #[test]
    #[bloomery("CLI-CHECK-PROGRESS-026")]
    fn queried_terminal_width_wins_over_the_columns_hint() {
        // A 60-column terminal exporting COLUMNS=80 must format to 60 so the
        // transient replacement never wraps.
        let facts = TerminalFacts {
            terminal_width: Some(60),
            columns: Some("80".to_owned()),
            ..TerminalFacts::default()
        };
        assert_eq!(facts.width(), 60);
    }

    #[test]
    fn width_falls_back_when_columns_is_missing_or_invalid() {
        assert_eq!(TerminalFacts::default().width(), 80);
        assert_eq!(
            TerminalFacts {
                columns: Some("120".to_owned()),
                ..TerminalFacts::default()
            }
            .width(),
            120
        );
        assert_eq!(
            TerminalFacts {
                columns: Some("0".to_owned()),
                ..TerminalFacts::default()
            }
            .width(),
            80
        );
        assert_eq!(
            TerminalFacts {
                columns: Some("wide".to_owned()),
                ..TerminalFacts::default()
            }
            .width(),
            80
        );
    }
}
