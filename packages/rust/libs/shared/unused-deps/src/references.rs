//! Crate names that source tokens reference as the start of a path.

use crate::cfg_test::attribute_requires_test;
use proc_macro2::{Delimiter, Spacing, TokenStream, TokenTree};
use std::collections::BTreeSet;

/// Names that start a path, split by whether the code is confined to tests.
#[derive(Debug, Default)]
pub(crate) struct References {
    /// Every referenced name.
    pub all: BTreeSet<String>,
    /// Names referenced outside code confined to `cfg(test)`.
    pub production: BTreeSet<String>,
}

impl References {
    /// Record the names `tokens` reference. `test_only` marks the whole stream
    /// as confined to `cfg(test)`.
    pub(crate) fn collect(&mut self, tokens: &TokenStream, test_only: bool) {
        let trees = tokens.clone().into_iter().collect::<Vec<_>>();
        self.collect_trees(&trees, test_only, &BTreeSet::new());
    }

    /// `modules` names the modules declared in enclosing code of the same
    /// module scope; relative paths through them are not crate references.
    fn collect_trees(
        &mut self,
        trees: &[TokenTree],
        mut test_only: bool,
        modules: &BTreeSet<String>,
    ) {
        let mut modules = modules.clone();
        modules.extend(declared_modules(trees));
        let mut index = 0;
        while index < trees.len() {
            if let Some((attribute_end, inner, requires_test)) = attribute_at(trees, index) {
                if inner && requires_test {
                    // `#![cfg(test)]` confines the rest of the enclosing module.
                    test_only = true;
                } else if requires_test && !test_only {
                    let item_end = item_end(trees, attribute_end);
                    self.collect_trees(&trees[index..item_end], true, &modules);
                    index = item_end;
                    continue;
                }
                self.collect_trees(&trees[index + 1..attribute_end], test_only, &modules);
                index = attribute_end;
                continue;
            }
            match &trees[index] {
                TokenTree::Group(group) => {
                    let inner = group.stream().into_iter().collect::<Vec<_>>();
                    if is_root_use_group(trees, index) {
                        for name in use_group_roots(&inner) {
                            self.insert(name, test_only);
                        }
                    }
                    // An inline module body starts a new module scope.
                    let enclosing = if is_module_body(trees, index) {
                        BTreeSet::new()
                    } else {
                        modules.clone()
                    };
                    self.collect_trees(&inner, test_only, &enclosing);
                }
                TokenTree::Ident(ident) if !is_keyword(ident) && starts_path(trees, index) => {
                    let name = ident_name(ident);
                    if !(modules.contains(&name) && is_relative_root(trees, index)) {
                        self.insert(name, test_only);
                    }
                }
                _ => {}
            }
            index += 1;
        }
    }

    fn insert(&mut self, name: String, test_only: bool) {
        if !test_only {
            self.production.insert(name.clone());
        }
        self.all.insert(name);
    }
}

fn ident_name(ident: &proc_macro2::Ident) -> String {
    let name = ident.to_string();
    name.strip_prefix("r#").unwrap_or(&name).to_owned()
}

/// Names of the `mod name;` and `mod name { ... }` items at this level.
fn declared_modules(trees: &[TokenTree]) -> BTreeSet<String> {
    trees
        .windows(2)
        .filter_map(|pair| match pair {
            [TokenTree::Ident(keyword), TokenTree::Ident(name)] if keyword == "mod" => {
                Some(ident_name(name))
            }
            _ => None,
        })
        .collect()
}

/// The brace group at `index` is the body of `mod name { ... }`.
fn is_module_body(trees: &[TokenTree], index: usize) -> bool {
    index >= 2
        && matches!(trees.get(index), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace)
        && matches!(trees.get(index - 1), Some(TokenTree::Ident(_)))
        && is_ident(trees.get(index - 2), "mod")
}

/// The brace group at `index` follows `use` or `use ::`, so each of its
/// entries starts at a crate root.
fn is_root_use_group(trees: &[TokenTree], index: usize) -> bool {
    let brace = matches!(trees.get(index), Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Brace);
    let previous = |offset: usize| index.checked_sub(offset).and_then(|at| trees.get(at));
    brace
        && (is_ident(previous(1), "use")
            || (index >= 3 && is_path_separator(trees, index - 2) && is_ident(previous(3), "use")))
}

/// The first identifier of every comma-separated entry of a root `use` group.
fn use_group_roots(trees: &[TokenTree]) -> Vec<String> {
    let mut roots = Vec::new();
    let mut entry_start = true;
    for tree in trees {
        match tree {
            TokenTree::Punct(punct) if punct.as_char() == ',' => entry_start = true,
            TokenTree::Ident(ident) if entry_start => {
                if !is_keyword(ident) {
                    roots.push(ident_name(ident));
                }
                entry_start = false;
            }
            _ => entry_start = false,
        }
    }
    roots
}

/// The identifier at `index` starts a relative `name::` path rather than a
/// global path, a `use` tree, or an `extern crate` item.
fn is_relative_root(trees: &[TokenTree], index: usize) -> bool {
    let previous = index.checked_sub(1).and_then(|at| trees.get(at));
    is_path_separator(trees, index + 1)
        && !(index >= 2 && is_path_separator(trees, index - 2))
        && !is_ident(previous, "use")
}

/// An attribute `#[...]` or `#![...]` starting at `index`: the index after
/// it, whether it is inner, and whether it confines its target to tests.
fn attribute_at(trees: &[TokenTree], index: usize) -> Option<(usize, bool, bool)> {
    if !is_punct(trees.get(index), '#') {
        return None;
    }
    let inner = is_punct(trees.get(index + 1), '!');
    let bracket = index + 1 + usize::from(inner);
    match trees.get(bracket) {
        Some(TokenTree::Group(group)) if group.delimiter() == Delimiter::Bracket => {
            Some((bracket + 1, inner, attribute_requires_test(group.stream())))
        }
        _ => None,
    }
}

/// End of the item, statement, field, or arm that starts at `start`: just
/// past the first `;`, `,`, or brace block at this nesting level.
fn item_end(trees: &[TokenTree], start: usize) -> usize {
    trees[start..]
        .iter()
        .position(|tree| match tree {
            TokenTree::Punct(punct) => matches!(punct.as_char(), ';' | ','),
            TokenTree::Group(group) => group.delimiter() == Delimiter::Brace,
            _ => false,
        })
        .map_or(trees.len(), |offset| start + offset + 1)
}

/// The identifier at `index` names a crate: it starts a relative path
/// (`name::`), follows a leading `::`, or is the root of a `use` tree or an
/// `extern crate` item.
fn starts_path(trees: &[TokenTree], index: usize) -> bool {
    let previous = index.checked_sub(1).and_then(|at| trees.get(at));
    if is_punct(previous, '.') {
        return false;
    }
    if index >= 2 && is_path_separator(trees, index - 2) {
        return is_global_path_start(trees, index - 2);
    }
    is_path_separator(trees, index + 1)
        || is_ident(previous, "use")
        || (is_ident(previous, "crate")
            && is_ident(index.checked_sub(2).and_then(|at| trees.get(at)), "extern"))
}

/// The `::` at `separator` starts a global path: no path segment, generic
/// argument list, or expression ends right before it. Keywords other than
/// path keywords, `->`, and `=>` do not end one.
fn is_global_path_start(trees: &[TokenTree], separator: usize) -> bool {
    let Some(before) = separator.checked_sub(1).and_then(|at| trees.get(at)) else {
        return true;
    };
    match before {
        TokenTree::Ident(ident) => {
            is_keyword(ident)
                && !matches!(
                    ident.to_string().as_str(),
                    "self" | "Self" | "super" | "crate"
                )
        }
        TokenTree::Punct(punct) if punct.as_char() == '>' => {
            let arrow = separator.checked_sub(2).and_then(|at| trees.get(at));
            matches!(arrow, Some(TokenTree::Punct(head))
                if matches!(head.as_char(), '-' | '=') && head.spacing() == Spacing::Joint)
        }
        TokenTree::Punct(_) => true,
        TokenTree::Group(_) | TokenTree::Literal(_) => false,
    }
}

/// Path keywords and other reserved words never name a crate; raw
/// identifiers such as `r#match` can.
fn is_keyword(ident: &proc_macro2::Ident) -> bool {
    matches!(
        ident.to_string().as_str(),
        "as" | "crate"
            | "dyn"
            | "extern"
            | "fn"
            | "for"
            | "impl"
            | "in"
            | "let"
            | "mut"
            | "pub"
            | "ref"
            | "return"
            | "self"
            | "Self"
            | "super"
            | "use"
            | "where"
    )
}

fn is_path_separator(trees: &[TokenTree], index: usize) -> bool {
    matches!(
        (trees.get(index), trees.get(index + 1)),
        (Some(TokenTree::Punct(first)), Some(TokenTree::Punct(second)))
            if first.as_char() == ':' && first.spacing() == Spacing::Joint && second.as_char() == ':'
    )
}

fn is_punct(tree: Option<&TokenTree>, expected: char) -> bool {
    matches!(tree, Some(TokenTree::Punct(punct)) if punct.as_char() == expected)
}

fn is_ident(tree: Option<&TokenTree>, expected: &str) -> bool {
    matches!(tree, Some(TokenTree::Ident(ident)) if ident == expected)
}

#[cfg(test)]
mod tests {
    use super::References;
    use std::str::FromStr;

    fn references(source: &str) -> References {
        let mut references = References::default();
        references.collect(
            &proc_macro2::TokenStream::from_str(source).expect("tokens"),
            false,
        );
        references
    }

    fn names(set: &std::collections::BTreeSet<String>) -> Vec<&str> {
        set.iter().map(String::as_str).collect()
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-012"))]
    fn only_path_roots_reference_crates() {
        let found = references(
            "use alpha;\nuse beta as _;\nextern crate gamma;\npub use ::delta::X;\n\
             fn f(log: u8) { let regex = log; epsilon::run(); self::zeta::g(); \
             outer::eta::h(); items.iter().theta::<u8>(); <T as Tr>::iota(); \
             #[kappa::attr] fn g() {} lambda!(); mu::nu!(); }\n\
             fn h() -> ::xi::T { match 0 { _ => ::omicron::f() } }\n\
             impl ::pi::Tr for S {} fn k(x: &dyn ::rho::Tr) -> Vec<u8>::sigma {}",
        );
        assert_eq!(
            names(&found.all),
            [
                "alpha", "beta", "delta", "epsilon", "gamma", "kappa", "mu", "omicron", "outer",
                "pi", "rho", "xi"
            ]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-014"))]
    fn root_use_group_entries_reference_crates() {
        let found = references(
            "use {alpha, beta::B, self::gamma};\nuse ::{delta as _, epsilon};\nuse zeta::{eta, theta};",
        );
        assert_eq!(
            names(&found.all),
            ["alpha", "beta", "delta", "epsilon", "zeta"]
        );
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-015"))]
    fn local_modules_shadow_relative_paths() {
        let found = references(
            "mod config;\nfn f() { config::load(); ::config::global(); }\n\
             mod inner { fn g() { config::load(); } }\n\
             mod other { mod log { } fn h() { log::info(); } }\n\
             use config as _;",
        );
        assert_eq!(names(&found.all), ["config"]);
        let shadowed = references("mod config;\nfn f() { config::load(); }");
        assert!(shadowed.all.is_empty(), "{:?}", shadowed.all);
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-013"))]
    fn cfg_test_items_are_test_only_references() {
        let found = references(
            "#[cfg(test)] mod tests { use alpha::A; }\n\
             #[cfg(all(test, unix))] use beta::B;\n\
             struct S { #[cfg(test)] a: gamma::G, b: delta::D }\n\
             #[cfg_attr(test, derive(Debug))] struct T(epsilon::E);\n\
             mod inner { #![cfg(test)] use zeta::Z; }\n\
             use eta::H;",
        );
        assert_eq!(
            names(&found.all),
            ["alpha", "beta", "delta", "epsilon", "eta", "gamma", "zeta"]
        );
        assert_eq!(names(&found.production), ["delta", "epsilon", "eta"]);
    }
}
