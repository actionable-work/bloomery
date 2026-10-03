use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn bloomery(_attribute: TokenStream, item: TokenStream) -> TokenStream {
    item
}
