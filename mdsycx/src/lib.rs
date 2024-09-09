//! # mdsycx
//!
//! **markdown with Sycamore**
//!
//! Plain ol’ markdown is a bit boring… What if we could spice it up with [Sycamore](https://sycamore-rs.netlify.app)?
//! Meet **mdsycx**!
//!
//! For more information, check out the [website](https://lukechu10.github.io/mdsycx/).

#![warn(missing_docs)]

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

/// An error returned from [`FromMd::set_prop`].
#[derive(Debug, Error)]
pub enum SetPropError {
    /// A prop with this name does not exist.
    /// 
    /// In markdown, props are stringly typed so the name must match exactly.
    #[error("a prop with this name does not exist")]
    UnknownProp,
    /// Could not parse the string into the prop type.
    /// 
    /// Parsing is performed using the [`FromStr`](std::str::FromStr) trait.
    #[error("could not parse value into prop type")]
    Parse,
}

/// Implemented by [`FromMd`](mdsycx_macro::FromMd) derive-macro.
pub trait FromMd: 'static {
    /// Create a new instance of the props.
    fn new_prop_default() -> Self;
    /// Set a prop by name. If a prop with the specified name does not exist or if the value could
    /// not be parsed, this returns an error.
    fn set_prop(&mut self, name: &str, value: &str) -> Result<(), SetPropError>;
    /// Set the `children` prop.
    fn set_children(&mut self, value: View);
}
