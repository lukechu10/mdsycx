//! # MDSycX (Markdown Sycamore Extensions)
//!
//! This crate allows you to easily render Markdown with Sycamore! Oh? You want to use your Sycamore
//! components in your Markdown? Don't worry, we got that covered!

mod components;
mod parser;

pub use components::*;
pub use parser::*;

pub use mdsycx_macro::*;

use sycamore::prelude::*;

/// Runtime support for the `mdsycx-macro` crate.
#[doc(hidden)]
pub mod rt {
    pub use serde;
}

/// Implemented by [`FromMd`](mdsycx_macro::FromMd) derive-macro.
pub trait FromMd<G: GenericNode> {
    fn new_prop_default() -> Self;
    fn set_prop(&mut self, name: &str, value: &str);
    fn set_children(&mut self, value: View<G>);
}
