use anyhow::Result;
use bat::PrettyPrinter;
use clap::{App, AppSettings, Arg};
use std::env;
use std::io;

mod chunk;
mod grep;
mod printer;
#[cfg(feature = "ripgrep")]
mod ripgrep;

use grep::BufReadExt;
use printer::{BatPrinter, Printer};

fn main() -> Result<()> {
    use anyhow::Context;

    let app = App::new("batgrep")
        .version(env!("CARGO_PKG_VERSION"))
        .about(
            "batgrep is like grep, but uses bat to print the results. It eats results of `grep -nH` and prints the matches with \
            syntax highlighting.\n\n\
            $ grep -nH pattern -R . | batgrep\n\n\
            For more details, visit https://github.com/rhysd/batgrep"
        )
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
            Arg::new("no-grid")
                .short('G')
                .long("no-grid")
                .about("Remove border lines for more compact output"),
        )
        .arg(
            Arg::new("tab")
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
        );

    #[cfg(feature = "ripgrep")]
    let app = app
            .about(
                "batgrep is like grep, but uses bat to print the results. It eats results of `grep -nH` and prints the matches \
                with syntax highlighting.\n\n\
                $ grep -nH pattern -R . | batgrep\n\n\
                batgrep has its builtin grep implementation. It's subset of ripgrep and faster when many matches are found.\n\n\
                $ batgrep pattern\n\n\
                For more details, visit https://github.com/rhysd/batgrep"
            )
            .override_usage("batgrep [FLAGS] [OPTIONS] [PATTERN [PATH...]]")
            .arg(
                Arg::new("no-ignore")
                    .long("no-ignore")
                    .about("Don't respect ignore files (.gitignore, .ignore, etc.)"),
            )
            .arg(
                Arg::new("ignore-case")
                    .short('i')
                    .long("ignore-case")
                    .about("When this flag is provided, the given patterns will be searched case insensitively"),
            )
            .arg(
                Arg::new("smart-case")
                    .short('S')
                    .long("smart-case")
                    .about("Search case insensitively if the pattern is all lowercase. Search case sensitively otherwise"),
            )
            .arg(Arg::new("hidden").long("hidden").about("Search hidden files and directories. By default, hidden files and directories are skipped"))
            .arg(
                Arg::new("glob")
                    .short('g')
                    .long("glob")
                    .takes_value(true)
                    .value_name("GLOB")
                    .multiple_values(true)
                    .allow_hyphen_values(true)
                    .about("Include or exclude files and directories for searching that match the given glob"),
            )
            .arg(
                Arg::new("glob-case-insensitive")
                    .long("glob-case-insensitive")
                    .about("Process glob patterns given with the -g/--glob flag case insensitively"),
            )
            .arg(
                Arg::new("fixed-strings")
                    .short('F')
                    .long("fixed-strings")
                    .about("Treat the pattern as a literal string instead of a regular expression"),
            )
            .arg(
                Arg::new("word-regexp")
                    .short('w')
                    .long("word-regexp")
                    .about("Only show matches surrounded by word boundaries"),
            )
            .arg(
                Arg::new("follow-symlink")
                    .short('L')
                    .long("follow")
                    .about("When this flag is enabled, batgrep will follow symbolic links while traversing directories"),
            )
            .arg(
                Arg::new("multiline")
                    .short('U')
                    .long("multiline")
                    .about("Enable matching across multiple lines"),
            )
            .arg(
                Arg::new("multiline-dotall")
                    .long("multiline-dotall")
                    .about("Enable \"dot all\" in your regex pattern, which causes '.' to match newlines when multiline searching is enabled"),
            )
            .arg(
                Arg::new("crlf")
                    .long("crlf")
                    .about(r"When enabled, batgrep will treat CRLF ('\r\n') as a line terminator instead of just '\n'"),
            )
            .arg(
                Arg::new("mmap")
                    .long("mmap")
                    .about("Search using memory maps when possible. mmap is disabled by default unlike batgrep"),
            )
            .arg(
                Arg::new("max-count")
                    .short('m')
                    .long("max-count")
                    .takes_value(true)
                    .value_name("NUM")
                    .about("Limit the number of matching lines per file searched to NUM"),
            )
            .arg(
                Arg::new("max-depth")
                    .long("max-depth")
                    .takes_value(true)
                    .value_name("NUM")
                    .about("Limit the depth of directory traversal to NUM levels beyond the paths given"),
            )
            .arg(
                Arg::new("max-filesize")
                    .long("max-filesize")
                    .takes_value(true)
                    .value_name("NUM")
                    .about("Ignore files larger than NUM in size"),
            )
            .arg(
                Arg::new("line-regexp")
                    .short('x')
                    .long("line-regexp")
                    .about("Only show matches surrounded by line boundaries. This is equivalent to putting ^...$ around all of the search patterns"),
            )
            .arg(
                Arg::new("pcre2")
                    .short('P')
                    .long("pcre2")
                    .about("When this flag is present, batgrep will use the PCRE2 regex engine instead of its default regex engine"),
            )
            .arg(Arg::new("PATTERN").about("Pattern to search. Regular expression is available"))
            .arg(Arg::new("PATH").about("Paths to search").multiple_values(true));

    let matches = app.get_matches();

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

    #[cfg(feature = "ripgrep")]
    {
        let pattern = matches.value_of("PATTERN");
        let paths = matches.values_of_os("PATH");
        let mut config = ripgrep::Config::default();
        config
            .context_lines(ctx)
            .no_ignore(matches.is_present("no-ignore"))
            .hidden(matches.is_present("hidden"))
            .case_insensitive(matches.is_present("ignore-case"))
            .smart_case(matches.is_present("smart-case"))
            .glob_case_insensitive(matches.is_present("glob-case-insensitive"))
            .pcre2(matches.is_present("pcre2")) // must be before fixed_string
            .fixed_strings(matches.is_present("fixed-strings"))
            .word_regexp(matches.is_present("word-regexp"))
            .follow_symlink(matches.is_present("follow-symlink"))
            .multiline(matches.is_present("multiline"))
            .crlf(matches.is_present("crlf"))
            .multiline_dotall(matches.is_present("multiline-dotall"))
            .mmap(matches.is_present("mmap"))
            .line_regexp(matches.is_present("line-regexp"));

        let globs = matches.values_of("glob");
        if let Some(globs) = globs {
            config.globs(globs);
        }

        if let Some(num) = matches.value_of("max-count") {
            let num = num
                .parse()
                .context("could not parse \"max-count\" option value as unsigned integer")?;
            config.max_count(num);
        }

        if let Some(num) = matches.value_of("max-depth") {
            let num = num
                .parse()
                .context("could not parse \"max-depth\" option value as unsigned integer")?;
            config.max_depth(num);
        }

        if let Some(num) = matches.value_of("max-filesize") {
            let num = num
                .parse()
                .context("could not parse \"max-filesize\" option value as unsigned integer")?;
            config.max_filesize(num);
        }

        match (pattern, paths) {
            (Some(pat), Some(paths)) => return ripgrep::grep(printer, pat, paths, config),
            (Some(pat), None) => {
                let cwd = env::current_dir()?;
                let paths = std::iter::once(cwd.as_os_str());
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
