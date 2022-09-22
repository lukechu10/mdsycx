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
use thiserror::Error;

/// Runtime support for the `mdsycx-macro` crate.
#[doc(hidden)]
pub mod rt {
    pub use serde;
}

#[derive(Debug, Error)]
pub enum SetPropError {
    #[error("a prop with this name does not exist")]
    UnknownProp,
    #[error("could not parse value into prop type")]
    Parse,
}

/// Implemented by [`FromMd`](mdsycx_macro::FromMd) derive-macro.
pub trait FromMd<G: GenericNode> {
    /// Create a new instance of the props.
    fn new_prop_default() -> Self;
    /// Set a prop by name. If a prop with the specified name does not exist or if the value could
    /// not be parsed, this returns an error.
    fn set_prop(&mut self, name: &str, value: &str) -> Result<(), SetPropError>;
    /// Set the `children` prop.
    fn set_children(&mut self, value: View<G>);
}
