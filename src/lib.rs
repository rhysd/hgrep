#[cfg(not(any(feature = "bat-printer", feature = "syntect-printer")))]
compile_error!("Either feature \"bat-printer\" or \"syntect-printer\" must be enabled");

pub mod chunk;
pub mod grep;
pub mod printer;

mod broken_pipe;

#[cfg(feature = "bat-printer")]
pub mod bat;
#[cfg(feature = "ripgrep")]
pub mod ripgrep;
#[cfg(feature = "syntect-printer")]
pub mod syntect;

#[cfg(test)]
mod test;

pub use anyhow::{Error, Result};
