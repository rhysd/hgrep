#![allow(clippy::new_without_default)]

#[cfg(not(any(feature = "bat-printer", feature = "syntect-printer")))]
compile_error!("Either feature \"bat-printer\" or \"syntect-printer\" must be enabled");

#[cfg(feature = "bat-printer")]
pub mod bat;
pub mod chunk;
pub mod grep;
mod io;
pub mod printer;
#[cfg(feature = "ripgrep")]
pub mod ripgrep;
#[cfg(feature = "syntect-printer")]
pub mod syntect;

#[cfg(test)]
mod test;

pub use anyhow::Error;
pub use anyhow::Result;
