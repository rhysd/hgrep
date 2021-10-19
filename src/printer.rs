use crate::chunk::File;
use anyhow::Result;

// Trait to replace printer implementation for unit tests
pub trait Printer {
    fn print(&self, file: File) -> Result<()>;
}
