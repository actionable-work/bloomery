//! Command-scoped Rust source ingestion shared by the scanners and health pass.

use proc_macro2::TokenStream;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Mutex;

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

/// Owns source reads and Rust syntax trees for one Bloomery command.
///
/// The cache includes failures. A second consumer therefore sees the same
/// source result and cannot trigger another read or parser invocation.
#[derive(Default)]
pub struct AnalysisSession {
    files: Mutex<HashMap<PathBuf, Result<Rc<ParsedRustFile>, String>>>,
    #[cfg(test)]
    parsed_files: AtomicUsize,
}

/// Parsed data is shared only within a command. Callers derive their compact
/// projections from it and drop the session when the command finishes.
pub struct ParsedRustFile {
    pub path: PathBuf,
    pub source: String,
    pub tokens: TokenStream,
    pub syntax: syn::File,
}

impl AnalysisSession {
    pub fn new() -> Self {
        Self::default()
    }

    #[cfg(test)]
    pub(crate) fn parse_count(&self) -> usize {
        self.parsed_files.load(Ordering::Relaxed)
    }

    pub fn rust_file(&self, path: &Path) -> Result<Rc<ParsedRustFile>, String> {
        let canonical = fs::canonicalize(path)
            .map_err(|error| format!("Unable to resolve Rust source: {error}"))?;
        let mut files = self
            .files
            .lock()
            .map_err(|_| "Rust analysis cache lock was poisoned".to_owned())?;
        if let Some(cached) = files.get(&canonical) {
            return cached.clone();
        }
        let parsed = (|| {
            let source = fs::read_to_string(&canonical)
                .map_err(|error| format!("Unable to read Rust source: {error}"))?;
            #[cfg(test)]
            self.parsed_files.fetch_add(1, Ordering::Relaxed);
            let tokens = TokenStream::from_str(&source)
                .map_err(|error| format!("Unable to tokenize Rust source: {error}"))?;
            let syntax = syn::parse2(tokens.clone())
                .map_err(|error| format!("Unable to parse Rust source: {error}"))?;
            Ok(Rc::new(ParsedRustFile {
                path: canonical.clone(),
                source,
                tokens,
                syntax,
            }))
        })();
        files.insert(canonical, parsed.clone());
        parsed
    }
}
