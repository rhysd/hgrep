use anyhow::Result;
use bat::PrettyPrinter;
use clap::{App, AppSettings, Arg};
use std::io;

mod chunk;
mod printer;
mod read;

use printer::Printer;
use read::BufReadExt;

fn main() -> Result<()> {
    use anyhow::Context;

    let matches = App::new("batgrep")
        .version(env!("CARGO_PKG_VERSION"))
        .about("like grep, but uses bat to show the results.")
        .global_setting(AppSettings::ColoredHelp)
        .arg(
            Arg::new("context")
                .short('c')
                .long("context")
                .takes_value(true)
                .value_name("NUM")
                .default_value("10")
                .about("Lines of leading and trailing context surrounding each match"),
        )
        .arg(
            Arg::new("tab")
                .short('t')
                .long("tab")
                .takes_value(true)
                .value_name("NUM")
                .about("Width of tab character"),
        )
        .arg(
            Arg::new("theme")
                .long("theme")
                .takes_value(true)
                .value_name("THEME")
                .about("Theme for syntax highlighting"),
        )
        .arg(
            Arg::new("list-themes")
                .long("list-themes")
                .about("List all theme names available for --theme option"),
        )
        .get_matches();

    if matches.is_present("list-themes") {
        for theme in PrettyPrinter::new().themes() {
            println!("{}", theme);
        }
        return Ok(());
    }

    let ctx = matches
        .value_of("context")
        .unwrap()
        .parse()
        .context("could not parse \"context\" option value as unsigned integer")?;

    let mut printer = Printer::new(ctx);

    if let Some(width) = matches.value_of("tab") {
        printer.tab_width(
            width
                .parse()
                .context("could not parse \"tab\" option value as unsigned integer")?,
        );
    }
    if let Some(theme) = matches.value_of("theme") {
        printer.theme(theme);
    }

    // XXX: io::stdin().lock() is not available since bat's implementation internally takes lock of stdin
    // *even if* it does not use stdin.
    // https://github.com/sharkdp/bat/issues/1902
    for c in io::BufReader::new(io::stdin()).grep_lines().chunks(ctx) {
        printer.print(c?)?;
    }
    Ok(())
}
