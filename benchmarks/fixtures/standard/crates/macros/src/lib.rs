//! Minimal proc-macro used to exercise the proc-macro build path.

use proc_macro::TokenStream;

/// Expand to the benchmark answer literal.
#[proc_macro]
pub fn value(_input: TokenStream) -> TokenStream {
    "42".parse().expect("valid integer literal")
}
