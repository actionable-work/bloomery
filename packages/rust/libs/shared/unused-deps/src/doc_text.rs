//! Crate names that documentation text, including doctests, starts paths with.

use crate::references::is_reserved;
use proc_macro2::{Literal, TokenStream, TokenTree};

/// The text of a `doc = "..."` attribute body.
pub(crate) fn doc_attribute_text(attribute: &TokenStream) -> Option<String> {
    let trees = attribute.clone().into_iter().collect::<Vec<_>>();
    match trees.as_slice() {
        [
            TokenTree::Ident(ident),
            TokenTree::Punct(punct),
            TokenTree::Literal(literal),
        ] if ident == "doc" && punct.as_char() == '=' => string_value(literal),
        _ => None,
    }
}

/// The value of a plain or raw string literal, with common escapes decoded.
fn string_value(literal: &Literal) -> Option<String> {
    let source = literal.to_string();
    if let Some(raw) = source.strip_prefix('r') {
        let hashes = raw.len() - raw.trim_start_matches('#').len();
        return raw
            .get(hashes + 1..raw.len().checked_sub(hashes + 1)?)
            .map(str::to_owned);
    }
    let inner = source.strip_prefix('"')?.strip_suffix('"')?;
    Some(
        inner
            .replace("\\n", "\n")
            .replace("\\\"", "\"")
            .replace("\\\\", "\\"),
    )
}

/// Names that start a `name::` path, follow `use`, or follow `extern crate`.
pub(crate) fn path_roots(text: &str) -> Vec<String> {
    let words = words(text);
    let mut roots = Vec::new();
    for (position, &(start, end)) in words.iter().enumerate() {
        let word = &text[start..end];
        if is_reserved(word) || word.starts_with(|c: char| c.is_ascii_digit()) {
            continue;
        }
        let previous = |back: usize| {
            position
                .checked_sub(back)
                .map(|at| &text[words[at].0..words[at].1])
        };
        let follows = |back: usize, keyword: &str| {
            previous(back) == Some(keyword)
                && text[words[position - back].1..words[position - back + 1].0]
                    .chars()
                    .all(char::is_whitespace)
        };
        let after_use = follows(1, "use") && starts_line(text, words[position - 1].0);
        let after_extern = follows(1, "crate") && follows(2, "extern");
        let starts_path = path_root_at(text, start, end);
        if starts_path || after_use || after_extern {
            roots.push(word.to_owned());
        }
    }
    roots
}

/// The word at `start..end` starts `name::` or a leading `::name` path.
fn path_root_at(text: &str, start: usize, end: usize) -> bool {
    let prefix = &text[..start];
    let (global, lead) = match prefix.strip_suffix("::") {
        Some(rest) => (true, rest.chars().next_back()),
        None => (false, prefix.chars().next_back()),
    };
    (global || text[end..].starts_with("::"))
        && !matches!(lead, Some(c) if c.is_alphanumeric() || matches!(c, '_' | '.' | ':' | '>' | '\'' | '"'))
}

/// Only whitespace, `#` (a hidden doctest line), or `pub` precede `at` on its
/// line.
fn starts_line(text: &str, at: usize) -> bool {
    let line = text[..at].rsplit('\n').next().unwrap_or_default();
    let line = line
        .trim_start_matches(|c: char| c.is_whitespace() || c == '#')
        .trim_end();
    line.is_empty() || line == "pub"
}

/// Byte ranges of identifier-like words.
fn words(text: &str) -> Vec<(usize, usize)> {
    let mut words = Vec::new();
    let mut start = None;
    for (index, character) in text.char_indices() {
        let word_char = character.is_alphanumeric() || character == '_';
        match (word_char, start) {
            (true, None) => start = Some(index),
            (false, Some(begin)) => {
                words.push((begin, index));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        words.push((begin, text.len()));
    }
    words
}

#[cfg(test)]
mod tests {
    use super::path_roots;

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-016"))]
    fn doc_text_path_roots_are_found() {
        let roots = path_roots(
            " ```\n # use alpha::A;\n let x = beta::run();\n extern crate gamma;\n \
             self::delta(); x.epsilon::<u8>(); \"zeta::y\" ::eta::f()\n ```\n \
             See [`theta::T`] and the use of iota.",
        );
        assert_eq!(roots, ["alpha", "beta", "gamma", "eta", "theta"]);
    }
}
