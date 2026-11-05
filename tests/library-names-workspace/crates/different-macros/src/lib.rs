extern crate proc_macro;
#[proc_macro]
pub fn answer(_: proc_macro::TokenStream) -> proc_macro::TokenStream {
    "42_u32".parse().expect("valid expansion")
}
