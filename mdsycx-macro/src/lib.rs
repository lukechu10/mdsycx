mod from_md;

use proc_macro::TokenStream;
use proc_macro_error::proc_macro_error;
use syn::parse_macro_input;

#[proc_macro_error]
#[proc_macro_derive(FromMd)]
pub fn derive_from_md(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as from_md::FromMdItem);

    from_md::from_md_impl(input).into()
}
