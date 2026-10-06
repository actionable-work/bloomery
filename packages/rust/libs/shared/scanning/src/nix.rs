use crate::ScanOutput;
use crate::files::expand_globs;
use bloomery_model::{Diagnostic, Evidence, SourceLocation, TestSite, config::NixScannerConfig};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(test)]
use std::sync::atomic::{AtomicUsize, Ordering};

#[cfg(test)]
pub(crate) static PARSE_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn scan(root: &Path, config: &NixScannerConfig) -> Result<ScanOutput, Vec<Diagnostic>> {
    let reference_paths = match expand_globs(root, &config.paths) {
        Ok(paths) => paths,
        Err(error) => return Err(vec![Diagnostic::new("ScannerError", error)]),
    };
    let test_paths = match expand_globs(root, &config.test_paths) {
        Ok(paths) => paths,
        Err(error) => return Err(vec![Diagnostic::new("ScannerError", error)]),
    };

    // Reference extraction reads `paths`; test discovery reads `testPaths`. The
    // union is scanned once so a file matched by both roles is parsed once.
    let mut wanted: BTreeMap<PathBuf, (bool, bool)> = BTreeMap::new();
    for path in reference_paths {
        wanted.entry(path).or_default().0 = true;
    }
    for path in test_paths {
        wanted.entry(path).or_default().1 = true;
    }

    let mut output = ScanOutput::default();
    let mut diagnostics = Vec::new();
    for (path, (_, is_test)) in wanted {
        let contents = match fs::read_to_string(&path) {
            Ok(contents) => contents,
            Err(error) => {
                diagnostics.push(
                    Diagnostic::new(
                        "NixScanError",
                        format!("Unable to read Nix source: {error}"),
                    )
                    .at(path, Some(1)),
                );
                continue;
            }
        };
        let (found, errors) = scan_source(root, &path, &contents);
        diagnostics.extend(errors);
        if is_test {
            let references = found.iter().map(|entry| entry.id.clone()).collect();
            let name = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(str::to_owned);
            output.tests.push(TestSite {
                scanner: "nix",
                name,
                location: SourceLocation::new(&path, Some(1)),
                references,
            });
        }
        output.evidence.extend(found);
    }
    if diagnostics.is_empty() {
        Ok(output)
    } else {
        Err(diagnostics)
    }
}

fn scan_source(root: &Path, path: &Path, source: &str) -> (Vec<Evidence>, Vec<Diagnostic>) {
    #[cfg(test)]
    PARSE_COUNT.fetch_add(1, Ordering::Relaxed);
    let tokens = tokenize(source);
    let mut evidence = Vec::new();
    let mut diagnostics = Vec::new();
    let mut index = 0;

    while index < tokens.len() {
        if is_direct_metadata_assignment(&tokens, index) {
            let (ids, errors, next) = metadata_rhs(&tokens, index + 4, path);
            evidence.extend(ids.into_iter().map(|(id, line)| Evidence {
                id,
                location: SourceLocation::new(path, Some(line)),
                scanner: "nix",
            }));
            diagnostics.extend(errors);
            index = next;
            continue;
        }

        if is_nested_passthru_assignment(&tokens, index) {
            let (ids, errors, next) = nested_metadata(&tokens, index + 2, path);
            evidence.extend(ids.into_iter().map(|(id, line)| Evidence {
                id,
                location: SourceLocation::new(path, Some(line)),
                scanner: "nix",
            }));
            diagnostics.extend(errors);
            index = next;
            continue;
        }
        index += 1;
    }

    if !path.starts_with(root) {
        diagnostics.push(
            Diagnostic::new(
                "NixScanError",
                "Nix source path escaped the repository root",
            )
            .at(path, Some(1)),
        );
    }
    (evidence, diagnostics)
}

fn is_direct_metadata_assignment(tokens: &[Token], index: usize) -> bool {
    matches!(
        tokens.get(index..index + 4),
        Some([
            Token {
                kind: TokenKind::Identifier(passthru),
                ..
            },
            Token {
                kind: TokenKind::Punctuation('.'),
                ..
            },
            Token {
                kind: TokenKind::Identifier(bloomery),
                ..
            },
            Token {
                kind: TokenKind::Punctuation('='),
                ..
            }
        ]) if passthru == "passthru" && bloomery == "bloomery"
    )
}

fn is_nested_passthru_assignment(tokens: &[Token], index: usize) -> bool {
    matches!(
        tokens.get(index..index + 3),
        Some([
            Token {
                kind: TokenKind::Identifier(passthru),
                ..
            },
            Token {
                kind: TokenKind::Punctuation('='),
                ..
            },
            Token {
                kind: TokenKind::Punctuation('{'),
                ..
            }
        ]) if passthru == "passthru"
    )
}

fn nested_metadata(
    tokens: &[Token],
    open_brace: usize,
    path: &Path,
) -> (Vec<(String, usize)>, Vec<Diagnostic>, usize) {
    let mut ids = Vec::new();
    let mut diagnostics = Vec::new();
    let mut depth = 1usize;
    let mut index = open_brace + 1;
    while index < tokens.len() && depth > 0 {
        if depth == 1 && is_nested_bloomery_assignment(tokens, index) {
            let (found, errors, next) = metadata_rhs(tokens, index + 2, path);
            ids.extend(found);
            diagnostics.extend(errors);
            index = next;
            continue;
        }
        match tokens[index].kind {
            TokenKind::Punctuation('{') => depth += 1,
            TokenKind::Punctuation('}') => depth -= 1,
            _ => {}
        }
        index += 1;
    }
    (ids, diagnostics, index)
}

fn is_nested_bloomery_assignment(tokens: &[Token], index: usize) -> bool {
    matches!(
        tokens.get(index..index + 2),
        Some([
            Token {
                kind: TokenKind::Identifier(bloomery),
                ..
            },
            Token {
                kind: TokenKind::Punctuation('='),
                ..
            }
        ]) if bloomery == "bloomery"
    )
}

fn metadata_rhs(
    tokens: &[Token],
    start: usize,
    path: &Path,
) -> (Vec<(String, usize)>, Vec<Diagnostic>, usize) {
    let mut ids = Vec::new();
    let mut diagnostics = Vec::new();
    let mut paren_depth = 0usize;
    let mut list_depth = 0usize;
    let mut attr_depth = 0usize;
    let mut index = start;

    while index < tokens.len() {
        let token = &tokens[index];
        match &token.kind {
            TokenKind::String(value) if list_depth > 0 => {
                if is_requirement_id(value) {
                    ids.push((value.clone(), token.line));
                } else {
                    diagnostics.push(
                        Diagnostic::new(
                            "NixReferenceError",
                            format!(
                                "passthru.bloomery contains a string that is not a requirement ID: {value}"
                            ),
                        )
                        .at(path, Some(token.line)),
                    );
                }
            }
            TokenKind::Punctuation(';')
                if paren_depth == 0 && list_depth == 0 && attr_depth == 0 =>
            {
                return (ids, diagnostics, index + 1);
            }
            TokenKind::Punctuation('(') => paren_depth += 1,
            TokenKind::Punctuation(')') => paren_depth = paren_depth.saturating_sub(1),
            TokenKind::Punctuation('[') => list_depth += 1,
            TokenKind::Punctuation(']') => list_depth = list_depth.saturating_sub(1),
            TokenKind::Punctuation('{') => attr_depth += 1,
            TokenKind::Punctuation('}') if attr_depth > 0 => attr_depth -= 1,
            TokenKind::Punctuation('}')
                if paren_depth == 0 && list_depth == 0 && attr_depth == 0 =>
            {
                return (ids, diagnostics, index);
            }
            _ => {}
        }
        index += 1;
    }
    (ids, diagnostics, index)
}

fn is_requirement_id(value: &str) -> bool {
    let segments = value.split('-').collect::<Vec<_>>();
    segments.len() == 4
        && segments[..3].iter().all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|character| {
                    character.is_ascii_uppercase() || character.is_ascii_digit() || character == '_'
                })
        })
        && (segments[3].len() == 3 || segments[3].len() == 4)
        && segments[3]
            .chars()
            .all(|character| character.is_ascii_digit())
}

#[derive(Debug)]
struct Token {
    kind: TokenKind,
    line: usize,
}

#[derive(Debug)]
enum TokenKind {
    Identifier(String),
    String(String),
    Punctuation(char),
}

fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut line = 1;

    while index < bytes.len() {
        if bytes[index].is_ascii_whitespace() {
            if bytes[index] == b'\n' {
                line += 1;
            }
            index += 1;
            continue;
        }
        if bytes[index] == b'#' {
            index = skip_line_comment(bytes, index);
            continue;
        }
        if bytes[index..].starts_with(b"/*") {
            index = skip_block_comment(bytes, index, &mut line);
            continue;
        }
        if bytes[index..].starts_with(b"''") {
            index = skip_indented_string(bytes, index, &mut line);
            continue;
        }
        if bytes[index] == b'"' {
            let token_line = line;
            let (value, next) = read_string(source, index, &mut line);
            if let Some(value) = value {
                tokens.push(Token {
                    kind: TokenKind::String(value),
                    line: token_line,
                });
            }
            index = next;
            continue;
        }

        let character = source[index..]
            .chars()
            .next()
            .expect("valid UTF-8 at token boundary");
        if identifier_start(character) {
            let start = index;
            index += character.len_utf8();
            while index < bytes.len() {
                let next = source[index..]
                    .chars()
                    .next()
                    .expect("valid UTF-8 at token boundary");
                if !identifier_continue(next) {
                    break;
                }
                index += next.len_utf8();
            }
            tokens.push(Token {
                kind: TokenKind::Identifier(source[start..index].to_owned()),
                line,
            });
            continue;
        }

        if character.is_ascii_punctuation() {
            tokens.push(Token {
                kind: TokenKind::Punctuation(character),
                line,
            });
        }
        index += character.len_utf8();
    }
    tokens
}

fn read_string(source: &str, start: usize, line: &mut usize) -> (Option<String>, usize) {
    let bytes = source.as_bytes();
    let mut index = start + 1;
    let content_start = index;
    while index < bytes.len() {
        let character = source[index..]
            .chars()
            .next()
            .expect("valid UTF-8 at string boundary");
        match character {
            '\\' => {
                index += character.len_utf8();
                if index < bytes.len() {
                    let escaped = source[index..]
                        .chars()
                        .next()
                        .expect("valid UTF-8 after string escape");
                    if escaped == '\n' {
                        *line += 1;
                    }
                    index += escaped.len_utf8();
                }
            }
            '"' => return (Some(source[content_start..index].to_owned()), index + 1),
            '\n' => {
                *line += 1;
                index += character.len_utf8();
            }
            _ => index += character.len_utf8(),
        }
    }
    (None, index)
}

fn skip_line_comment(bytes: &[u8], mut index: usize) -> usize {
    while index < bytes.len() && bytes[index] != b'\n' {
        index += 1;
    }
    index
}

fn skip_block_comment(bytes: &[u8], mut index: usize, line: &mut usize) -> usize {
    let mut depth = 1usize;
    index += 2;
    while index < bytes.len() && depth > 0 {
        if bytes[index..].starts_with(b"/*") {
            depth += 1;
            index += 2;
        } else if bytes[index..].starts_with(b"*/") {
            depth -= 1;
            index += 2;
        } else {
            if bytes[index] == b'\n' {
                *line += 1;
            }
            index += 1;
        }
    }
    index
}

fn skip_indented_string(bytes: &[u8], mut index: usize, line: &mut usize) -> usize {
    index += 2;
    while index < bytes.len() {
        if bytes[index..].starts_with(b"'''") {
            index += 3;
        } else if bytes[index..].starts_with(b"''${") {
            index += 4;
        } else if bytes[index..].starts_with(b"''\"") {
            index += 3;
        } else if bytes[index..].starts_with(b"''\\") && index + 3 < bytes.len() {
            if bytes[index + 3] == b'\n' {
                *line += 1;
            }
            index += 4;
        } else if bytes[index..].starts_with(b"''") {
            return index + 2;
        } else {
            if bytes[index] == b'\n' {
                *line += 1;
            }
            index += 1;
        }
    }
    index
}

fn identifier_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}

fn identifier_continue(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '-' | '\'')
}
