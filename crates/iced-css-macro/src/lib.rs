use proc_macro::TokenStream;

#[proc_macro_attribute]
pub fn style(args: TokenStream, item: TokenStream) -> TokenStream {
    let _ = args;
    item
}
