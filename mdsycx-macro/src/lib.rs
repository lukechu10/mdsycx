mod from_md;

use proc_macro::TokenStream;
use syn::parse_macro_input;

#[proc_macro_derive(FromMd)]
pub fn derive_from_md(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as from_md::FromMdItem);

    from_md::from_md_impl(input)
        .unwrap_or_else(|err| err.into_compile_error())
        .into()
}
