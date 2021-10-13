use anyhow::Result;
use bat::PrettyPrinter;
use clap::{App, AppSettings, Arg};
use std::env;
use std::io;

#[cfg(not(feature = "ripgrep"))]
use std::fmt;

#[cfg(feature = "ripgrep")]
use std::iter;

mod chunk;
mod grep;
mod printer;
#[cfg(feature = "ripgrep")]
mod ripgrep;

use grep::BufReadExt;
use printer::{BatPrinter, Printer};

#[cfg(not(feature = "ripgrep"))]
#[derive(Debug)]
enum CommandError {
    PathArgNotSupported(&'static str),
}

#[cfg(not(feature = "ripgrep"))]
impl fmt::Display for CommandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CommandError::PathArgNotSupported => write!(
                f,
                "PATH argument is not supported without \"ripgrep\" feature"
            ),
        }
    }
}

#[cfg(not(feature = "ripgrep"))]
impl std::error::Error for CommandError {}

fn main() -> Result<()> {
    use anyhow::Context;

    macro_rules! ripgrep_only_descriptions {
        ($($ident:ident = $desc:expr,)+) => {
            $(
                #[cfg(feature = "ripgrep")]
                let $ident = $desc;
                #[cfg(not(feature = "ripgrep"))]
                let $ident = "This option is for the builtin \"ripgrep\" feature. Not available until installing batgrep with the feature";
            )+
        };
    }

    ripgrep_only_descriptions! {
        path_desc = "Paths to search",
        pattern_desc = "Pattern to search. Regular expression is available",
        no_ignore_desc = "Don't respect ignore files (.gitignore, .ignore, etc.)",
        hidden_desc = "Search hidden files and directories. By default, hidden files and directories are skipped",
        ignore_case_desc = "When this flag is provided, the given patterns will be searched case insensitively",
        smart_case_desc = "Searches case insensitively if the pattern is all lowercase. Search case sensitively otherwise",
        glob_desc = "Include or exclude files and directories for searching that match the given glob",
        glob_case_insensitive_desc = "Process glob patterns given with the -g/--glob flag case insensitively",
        fixed_strings_desc = "Treat the pattern as a literal string instead of a regular expression",
        word_regexp_desc = "Only show matches surrounded by word boundaries",
        follow_symlink_desc = "When this flag is enabled, ripgrep will follow symbolic links while traversing directories",
    }

    let matches = App::new("batgrep")
        .version(env!("CARGO_PKG_VERSION"))
        .about("like grep, but uses bat to show the results.")
        .global_setting(AppSettings::ColoredHelp)
        .override_usage("batgrep [FLAGS] [OPTIONS] [PATTERN [PATH...]]")
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
            Arg::new("no-grid")
                .short('G')
                .long("no-grid")
                .about("Remove border lines for more compact output"),
        )
        .arg(
            Arg::new("tab")
                .short('t')
                .long("tab")
                .takes_value(true)
                .value_name("NUM")
                .about("Number of spaces for tab character"),
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
        .arg(
            Arg::new("no-ignore")
                .long("no-ignore")
                .about(no_ignore_desc),
        )
        .arg(
            Arg::new("ignore-case")
                .short('i')
                .long("ignore-case")
                .about(ignore_case_desc),
        )
        .arg(
            Arg::new("smart-case")
                .short('S')
                .long("smart-case")
                .about(smart_case_desc),
        )
        .arg(Arg::new("hidden").long("hidden").about(hidden_desc))
        .arg(
            Arg::new("glob")
                .short('g')
                .long("glob")
                .value_name("GLOB")
                .takes_value(true)
                .multiple_values(true)
                .allow_hyphen_values(true)
                .about(glob_desc),
        )
        .arg(
            Arg::new("glob-case-insensitive")
                .long("glob-case-insensitive")
                .about(glob_case_insensitive_desc),
        )
        .arg(
            Arg::new("fixed-strings")
                .short('F')
                .long("fixed-strings")
                .about(fixed_strings_desc),
        )
        .arg(
            Arg::new("word-regexp")
                .short('w')
                .long("word-regexp")
                .about(word_regexp_desc),
        )
        .arg(
            Arg::new("follow-symlink")
                .short('L')
                .long("follow")
                .about(follow_symlink_desc),
        )
        .arg(Arg::new("PATTERN").about(pattern_desc))
        .arg(Arg::new("PATH").about(path_desc).multiple_values(true))
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

    let mut printer = BatPrinter::new(ctx);

    if let Some(width) = matches.value_of("tab") {
        printer.tab_width(
            width
                .parse()
                .context("could not parse \"tab\" option value as unsigned integer")?,
        );
    }

    let theme_env = env::var("BAT_THEME").ok();
    if let Some(var) = &theme_env {
        printer.theme(var);
    }
    if let Some(theme) = matches.value_of("theme") {
        printer.theme(theme);
    }

    if let Ok("plain" | "header" | "numbers") = env::var("BAT_STYLE").as_ref().map(String::as_str) {
        printer.grid(false);
    }
    if matches.is_present("no-grid") {
        printer.grid(false);
    }

    let pattern = matches.value_of("PATTERN");
    #[cfg(not(feature = "ripgrep"))]
    if pattern.is_some() {
        return Err(CommandError::PathArgNotSupported("PATTERN").into());
    }

    let paths = matches.values_of_os("PATH");
    #[cfg(not(feature = "ripgrep"))]
    if paths.is_some() {
        return Err(CommandError::PathArgNotSupported("PATH").into());
    }

    #[cfg(feature = "ripgrep")]
    {
        let mut config = ripgrep::Config::new(ctx);
        config
            .no_ignore(matches.is_present("no-ignore"))
            .hidden(matches.is_present("hidden"))
            .case_insensitive(matches.is_present("ignore-case"))
            .smart_case(matches.is_present("smart-case"))
            .glob_case_insensitive(matches.is_present("glob-case-insensitive"))
            .fixed_strings(matches.is_present("fixed-strings"))
            .word_regexp(matches.is_present("word-regexp"))
            .follow_symlink(matches.is_present("follow-symlink"));
        let globs = matches.values_of("glob");
        if let Some(globs) = globs {
            config.globs(globs);
        }
        match (pattern, paths) {
            (Some(pat), Some(paths)) => return ripgrep::grep(printer, pat, paths, config),
            (Some(pat), None) => {
                let cwd = env::current_dir()?;
                let paths = iter::once(cwd.as_os_str());
                return ripgrep::grep(printer, pat, paths, config);
            }
            _ => { /* fall through */ }
        }
    }

    // XXX: io::stdin().lock() is not available since bat's implementation internally takes lock of stdin
    // *even if* it does not use stdin.
    // https://github.com/sharkdp/bat/issues/1902
    for c in io::BufReader::new(io::stdin()).grep_lines().chunks(ctx) {
        printer.print(c?)?;
    }

    Ok(())
}
