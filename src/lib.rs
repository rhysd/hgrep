#![allow(clippy::new_without_default)]

pub mod chunk;
pub mod grep;
pub mod printer;
#[cfg(feature = "ripgrep")]
pub mod ripgrep;

#[cfg(test)]
mod test;

pub use anyhow::Error;
pub use anyhow::Result;
