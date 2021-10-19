#![allow(clippy::new_without_default)]

pub mod bat;
pub mod chunk;
pub mod grep;
pub mod printer;
#[cfg(feature = "ripgrep")]
pub mod ripgrep;
#[cfg(feature = "syntect-printer")]
pub mod syntect;

#[cfg(test)]
mod test;

pub use anyhow::Error;
pub use anyhow::Result;
