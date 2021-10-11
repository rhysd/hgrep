use anyhow::Result;
use std::io;

mod chunk;
mod printer;
mod read;

use printer::Printer;
use read::BufReadExt;

fn main() -> Result<()> {
    let ctx = 10;
    let printer = Printer::new(ctx);
    // XXX: io::stdin().lock() is not available since bat's implementation internally takes lock of stdin
    // *even if* it does not use stdin.
    // https://github.com/sharkdp/bat/issues/1902
    for c in io::BufReader::new(io::stdin()).grep_lines().chunks(ctx) {
        printer.print(c?)?;
    }
    Ok(())
}
