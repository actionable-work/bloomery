use std::cmp::Ordering;
use std::fmt;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceLocation {
    pub path: PathBuf,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl SourceLocation {
    pub fn new(path: impl Into<PathBuf>, line: Option<usize>) -> Self {
        Self {
            path: path.into(),
            line,
            column: None,
        }
    }

    pub fn display_path(&self, root: &Path) -> PathBuf {
        self.path
            .strip_prefix(root)
            .map(Path::to_path_buf)
            .unwrap_or_else(|_| self.path.clone())
    }
}

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub location: Option<SourceLocation>,
    pub notes: Vec<String>,
}

impl Diagnostic {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
            location: None,
            notes: Vec::new(),
        }
    }

    pub fn at(mut self, path: impl Into<PathBuf>, line: Option<usize>) -> Self {
        self.location = Some(SourceLocation::new(path, line));
        self
    }

    pub fn note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }
}

pub fn sort_diagnostics(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| {
        fn location_key(diagnostic: &Diagnostic) -> Option<(&Path, usize)> {
            diagnostic
                .location
                .as_ref()
                .map(|location| (location.path.as_path(), location.line.unwrap_or(0)))
        }

        match (location_key(left), location_key(right)) {
            (Some((left_path, left_line)), Some((right_path, right_line))) => {
                left_path.cmp(right_path).then(left_line.cmp(&right_line))
            }
            (None, Some(_)) => Ordering::Less,
            (Some(_), None) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        }
        .then_with(|| left.code.cmp(&right.code))
        .then_with(|| left.message.cmp(&right.message))
    });
}

pub fn render_diagnostics(root: &Path, diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();
    for diagnostic in diagnostics {
        output.push_str(&format!(
            "ERROR [{}]:\n  {}\n",
            diagnostic.code, diagnostic.message
        ));
        if let Some(location) = &diagnostic.location {
            let path = location.display_path(root);
            let line = location.line.unwrap_or(1);
            let column = location
                .column
                .map(|value| format!(":{value}"))
                .unwrap_or_default();
            output.push_str(&format!("  --> {}:{}{}\n", path.display(), line, column));
        }
        for note in &diagnostic.notes {
            output.push_str(&format!("  = note: {note}\n"));
        }
        output.push('\n');
    }
    output.push_str(&format!(
        "FAIL: {} error{} detected.\n",
        diagnostics.len(),
        if diagnostics.len() == 1 { "" } else { "s" }
    ));
    output
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}
