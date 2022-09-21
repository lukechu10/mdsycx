use proc_macro2::TokenStream;
use proc_macro_error::abort;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::spanned::Spanned;
use syn::{Error, GenericArgument, Item, ItemStruct, PathArguments, Type};

pub struct FromMdItem {
    item: ItemStruct,
}

impl Parse for FromMdItem {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let parsed: Item = input.parse()?;
        match parsed {
            Item::Struct(s) => Ok(Self { item: s }),
            _ => Err(Error::new(
                parsed.span(),
                "the `FromMd` derive macro can only be applied to structs",
            )),
        }
    }
}

pub fn from_md_impl(input: FromMdItem) -> TokenStream {
    let struct_ident = &input.item.ident;
    let (impl_generics, ty_generics, where_clause) = input.item.generics.split_for_impl();

    let fields = match &input.item.fields {
        syn::Fields::Named(fields) => fields.named.iter().collect(),
        syn::Fields::Unnamed(_) => abort!(
            input.item,
            "the `FromMd` derive macro cannot be applied to tuple structs"
        ),
        syn::Fields::Unit => Vec::new(),
    };
    // We need to get the generic node type from the children field.
    let children_field = fields
        .iter()
        .find(|f| f.ident.as_ref().unwrap() == "children");
    let generic_node_ty;
    if let Some(children_field) = children_field {
        match &children_field.ty {
            Type::Path(ty) => {
                let last = ty.path.segments.last().unwrap();
                match &last.arguments {
                    PathArguments::None => {
                        abort!(last, "the `children` prop should have a generic type")
                    }
                    PathArguments::AngleBracketed(args) => {
                        generic_node_ty = args
                            .args
                            .iter()
                            .find(|arg| matches!(arg, GenericArgument::Type(_)))
                    }
                    PathArguments::Parenthesized(args) => {
                        abort!(args, "wrong type of generics for the `children` prop")
                    }
                };
            }
            _ => abort!(children_field.ty, "invalid type for `children` prop"),
        }
    } else {
        abort!(
            input.item,
            "the `children` prop is required but was not found"
        );
    }
    // Remove the `children` prop from `fields` because it is handled specially.
    let fields = fields
        .into_iter()
        .filter(|f| f.ident.as_ref().unwrap() != "children")
        .collect::<Vec<_>>();

    let idents = fields
        .iter()
        .map(|f| f.ident.as_ref().unwrap())
        .collect::<Vec<_>>();
    let idents_str = idents.iter().map(|id| id.to_string()).collect::<Vec<_>>();
    let idents_ty = fields.iter().map(|f| &f.ty).collect::<Vec<_>>();
    assert_eq!(idents_str.len(), idents_ty.len());

    quote! {
        impl #impl_generics ::mdsycx::FromMd<#generic_node_ty> for #struct_ident #ty_generics #where_clause {
            fn new_prop_default() -> Self {
                Self {
                    #(
                        #idents: ::std::default::Default::default(),
                    )*
                    children: ::std::default::Default::default(),
                }
            }

            fn set_prop(&mut self, name: &::std::primitive::str, value: &::std::primitive::str) {
                match name {
                    #(
                    #idents_str => {
                        let data: #idents_ty = ::std::str::FromStr::from_str(value).unwrap();
                        self.#idents = data;
                    }
                    )*
                    _ => ::std::panic!("unknown prop"),
                }
            }

            fn set_children(&mut self, value: ::sycamore::view::View<#generic_node_ty>) {
                self.children = ::sycamore::component::Children::from(value);
            }
        }
    }
}
