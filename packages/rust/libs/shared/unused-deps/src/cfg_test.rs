//! Whether `cfg` predicates confine code to test builds.

use proc_macro2::{Delimiter, TokenStream, TokenTree};

/// True when the `cfg` predicate holds only when `test` is set, such as
/// `test`, `all(test, unix)`, or `any(test, all(test, unix))`.
pub(crate) fn predicate_requires_test(predicate: &TokenStream) -> bool {
    let trees = predicate.clone().into_iter().collect::<Vec<_>>();
    match trees.as_slice() {
        [TokenTree::Ident(ident)] => ident == "test",
        [TokenTree::Ident(ident), TokenTree::Group(group)]
            if group.delimiter() == Delimiter::Parenthesis =>
        {
            let items = split_commas(group.stream());
            if ident == "all" {
                items.iter().any(predicate_requires_test)
            } else if ident == "any" {
                !items.is_empty() && items.iter().all(predicate_requires_test)
            } else {
                false
            }
        }
        _ => false,
    }
}

/// True when the tokens inside an attribute's brackets are `cfg(predicate)`
/// with a predicate that requires `test`.
pub(crate) fn attribute_requires_test(attribute: TokenStream) -> bool {
    let trees = attribute.into_iter().collect::<Vec<_>>();
    matches!(
        trees.as_slice(),
        [TokenTree::Ident(ident), TokenTree::Group(group)]
            if ident == "cfg"
                && group.delimiter() == Delimiter::Parenthesis
                && predicate_requires_test(&group.stream())
    )
}

fn split_commas(tokens: TokenStream) -> Vec<TokenStream> {
    let mut items = vec![TokenStream::new()];
    for tree in tokens {
        match &tree {
            TokenTree::Punct(punct) if punct.as_char() == ',' => items.push(TokenStream::new()),
            _ => items.last_mut().expect("item").extend([tree]),
        }
    }
    items.retain(|item| !item.is_empty());
    items
}

#[cfg(test)]
mod tests {
    use super::attribute_requires_test;
    use std::str::FromStr;

    fn requires_test(attribute: &str) -> bool {
        attribute_requires_test(proc_macro2::TokenStream::from_str(attribute).expect("tokens"))
    }

    #[test]
    #[cfg_attr(any(), bloomery("CLI-CHECK-UNUSED-013"))]
    fn only_predicates_implying_test_are_test_scoped() {
        assert!(requires_test("cfg(test)"));
        assert!(requires_test("cfg(all(unix, test))"));
        assert!(requires_test("cfg(any(test, all(test, unix)))"));
        assert!(!requires_test("cfg(any(test, unix))"));
        assert!(!requires_test("cfg(any())"));
        assert!(!requires_test("cfg(not(test))"));
        assert!(!requires_test("cfg_attr(test, allow(dead_code))"));
        assert!(!requires_test("cfg(feature = \"test\")"));
        assert!(!requires_test("cfg(unix)"));
    }
}
