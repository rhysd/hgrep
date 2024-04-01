#![deny(clippy::dbg_macro)]

use anyhow::{Context, Result};
use clap::{Arg, ArgAction, ArgMatches, Command};
use hgrep::grep::BufReadExt;
use hgrep::printer::{PrinterOptions, TextWrapMode};
use std::cmp;
use std::env;
use std::ffi::OsString;
use std::io;
use std::process;

#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(feature = "ripgrep")]
use hgrep::ripgrep;

#[cfg(feature = "bat-printer")]
use hgrep::bat::BatPrinter;

#[cfg(feature = "syntect-printer")]
use hgrep::syntect::SyntectPrinter;

const COMPLETION_SHELLS: [&str; 6] = ["bash", "zsh", "powershell", "fish", "elvish", "nushell"];
const OPTS_ENV_VAR: &str = "HGREP_DEFAULT_OPTS";

#[derive(Debug)]
struct Args {
    env: Vec<String>,
    args: env::ArgsOs,
}

impl Args {
    fn new() -> Result<Self> {
        let env = match env::var(OPTS_ENV_VAR) {
            Ok(var) => {
                let Some(mut opts) = shlex::split(&var) else {
                    anyhow::bail!("String in `{}` environment variable cannot be parsed as a shell command: {:?}", OPTS_ENV_VAR, var);
                };
                opts.reverse();
                opts
            }
            Err(env::VarError::NotPresent) => vec![],
            Err(env::VarError::NotUnicode(invalid)) => {
                anyhow::bail!(
                    "String in `{}` environment variable is not a valid UTF-8 sequence: {:?}",
                    OPTS_ENV_VAR,
                    invalid,
                );
            }
        };

        let mut args = env::args_os();
        args.next(); // Skip the executable name at the first item

        Ok(Self { env, args })
    }
}

impl Iterator for Args {
    type Item = OsString;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(arg) = self.env.pop() {
            Some(arg.into())
        } else {
            self.args.next()
        }
    }
}

fn command() -> Command {
    #[cfg(feature = "syntect-printer")]
    const DEFAULT_PRINTER: &str = "syntect";

    #[cfg(all(not(feature = "syntect-printer"), feature = "bat-printer"))]
    const DEFAULT_PRINTER: &str = "bat";

    let cmd = Command::new("hgrep")
        .version(env!("CARGO_PKG_VERSION"))
        .about(
            "hgrep is grep with human-friendly search output. hgrep eats an output of `grep -nH` and prints the matches \
            with syntax-highlighted code snippets.\n\n\
            $ grep -nH pattern -R . | hgrep\n\n\
            The default options can be customized with HGREP_DEFAULT_OPTS environment variable. \
            For more details, visit https://github.com/rhysd/hgrep#readme"
        )
        .no_binary_name(true)
        .arg(
            Arg::new("min-context")
                .short('c')
                .long("min-context")
                .num_args(1)
                .value_name("NUM")
                .default_value("3")
                .help("Minimum lines of leading and trailing context surrounding each match"),
        )
        .arg(
            Arg::new("max-context")
                .short('C')
                .long("max-context")
                .num_args(1)
                .value_name("NUM")
                .default_value("6")
                .help("Maximum lines of leading and trailing context surrounding each match"),
        )
        .arg(
            Arg::new("no-grid")
                .short('G')
                .long("no-grid")
                .action(ArgAction::SetTrue)
                .help("Remove borderlines for more compact output"),
        )
        .arg(
            Arg::new("grid")
                .long("grid")
                .action(ArgAction::SetTrue)
                .help("Add borderlines to output. This flag is an opposite of --no-grid"),
        )
        .arg(
            Arg::new("tab")
                .long("tab")
                .num_args(1)
                .value_name("NUM")
                .default_value("4")
                .help("Number of spaces for tab character. Set 0 to pass tabs through directly"),
        )
        .arg(
            Arg::new("theme")
                .long("theme")
                .num_args(1)
                .value_name("THEME")
                .help("Theme for syntax highlighting. Use --list-themes flag to print the theme list"),
        )
        .arg(
            Arg::new("list-themes")
                .long("list-themes")
                .action(ArgAction::SetTrue)
                .help("List all available theme names and their samples. Samples show the output where 'let' is searched. The names can be used at --theme option"),
        )
        .arg(
            Arg::new("printer")
                .short('p')
                .long("printer")
                .value_name("PRINTER")
                .default_value(DEFAULT_PRINTER)
                .value_parser([
                    #[cfg(feature = "syntect-printer")]
                    "syntect",
                    #[cfg(feature = "bat-printer")]
                    "bat",
                ])
                .help("Printer to print the match results"),
        )
        .arg(
            Arg::new("term-width")
                .long("term-width")
                .num_args(1)
                .value_name("NUM")
                .help("Width (number of characters) of terminal window"),
        ).arg(
            Arg::new("wrap")
                .long("wrap")
                .num_args(1)
                .value_name("MODE")
                .default_value("char")
                .value_parser(["char", "never"])
                .ignore_case(true)
                .help("Text-wrapping mode. 'char' enables character-wise text-wrapping. 'never' disables text-wrapping")
        ).arg(
            Arg::new("first-only")
                .short('f')
                .long("first-only")
                .action(ArgAction::SetTrue)
                .help("Show only the first code snippet per file")
        )
        .arg(
            Arg::new("generate-completion-script")
                .long("generate-completion-script")
                .num_args(1)
                .value_name("SHELL")
                .value_parser(COMPLETION_SHELLS)
                .ignore_case(true)
                .help("Print completion script for SHELL to stdout"),
        )
        .arg(
            Arg::new("generate-man-page")
                .long("generate-man-page")
                .action(ArgAction::SetTrue)
                .help("Print man page to stdout"),
        );

    #[cfg(feature = "bat-printer")]
    let cmd = cmd.arg(
        Arg::new("custom-assets")
            .long("custom-assets")
            .action(ArgAction::SetTrue)
            .help("Load bat's custom assets. Note that this flag may not work with some version of `bat` command. This flag is only for bat printer"),
    );

    #[cfg(feature = "syntect-printer")]
    let cmd = cmd
        .arg(
            Arg::new("background")
                .long("background")
                .action(ArgAction::SetTrue)
                .help("Paint background colors. This flag is only for syntect printer"),
        )
        .arg(
            Arg::new("ascii-lines")
                .long("ascii-lines")
                .action(ArgAction::SetTrue)
                .help(
                    "Use ASCII characters for drawing border lines instead of Unicode characters",
                ),
        );

    #[cfg(feature = "ripgrep")]
    let cmd = cmd
            .about(
                "hgrep is grep with human-friendly search output.\n\n\
                hgrep eats an output of `grep -nH` and prints the matches with syntax-highlighted code snippets.\n\n\
                $ grep -nH pattern -R . | hgrep\n\n\
                hgrep has its builtin subset of ripgrep, whose search output and performance are better than reading \
                the output from `grep -nH`.\n\n\
                $ hgrep pattern\n\n\
                The default options can be customized with HGREP_DEFAULT_OPTS environment variable. \
                For more details, visit https://github.com/rhysd/hgrep#readme"
            )
            .override_usage("hgrep [FLAGS] [OPTIONS] [PATTERN [PATH...]]")
            .arg(
                Arg::new("no-ignore")
                    .long("no-ignore")
                    .action(ArgAction::SetTrue)
                    .help("Don't respect ignore files (.gitignore, .ignore, etc.)"),
            )
            .arg(
                Arg::new("ignore-case")
                    .short('i')
                    .long("ignore-case")
                    .action(ArgAction::SetTrue)
                    .overrides_with("smart-case")
                    .help("When this flag is provided, the given pattern will be searched case insensitively. This flag overrides --smart-case"),
            )
            .arg(
                Arg::new("smart-case")
                    .short('S')
                    .long("smart-case")
                    .action(ArgAction::SetTrue)
                    .overrides_with("ignore-case")
                    .help("Search case insensitively if the pattern is all lowercase. Search case sensitively otherwise. This flag overrides --ignore-case"),
            )
            .arg(
                Arg::new("hidden")
                    .short('.')
                    .long("hidden")
                    .action(ArgAction::SetTrue)
                    .help("Search hidden files and directories. By default, hidden files and directories are skipped"),
            )
            .arg(
                Arg::new("glob")
                    .short('g')
                    .long("glob")
                    .action(ArgAction::Append)
                    .num_args(1)
                    .value_name("GLOB")
                    .allow_hyphen_values(true)
                    .help("Include or exclude files and directories for searching that match the given glob"),
            )
            .arg(
                Arg::new("glob-case-insensitive")
                    .long("glob-case-insensitive")
                    .action(ArgAction::SetTrue)
                    .help("Process glob patterns given with the -g/--glob flag case insensitively"),
            )
            .arg(
                Arg::new("fixed-strings")
                    .short('F')
                    .long("fixed-strings")
                    .action(ArgAction::SetTrue)
                    .help("Treat the pattern as a literal string instead of a regular expression"),
            )
            .arg(
                Arg::new("word-regexp")
                    .short('w')
                    .long("word-regexp")
                    .action(ArgAction::SetTrue)
                    .overrides_with("line-regexp")
                    .help("Only show matches surrounded by word boundaries. This flag overrides --line-regexp"),
            )
            .arg(
                Arg::new("follow-symlink")
                    .short('L')
                    .long("follow")
                    .action(ArgAction::SetTrue)
                    .help("When this flag is enabled, hgrep will follow symbolic links while traversing directories"),
            )
            .arg(
                Arg::new("multiline")
                    .short('U')
                    .long("multiline")
                    .action(ArgAction::SetTrue)
                    .help("Enable matching across multiple lines"),
            )
            .arg(
                Arg::new("multiline-dotall")
                    .long("multiline-dotall")
                    .action(ArgAction::SetTrue)
                    .help("Enable \"dot all\" in your regex pattern, which causes '.' to match newlines when multiline searching is enabled"),
            )
            .arg(
                Arg::new("crlf")
                    .long("crlf")
                    .action(ArgAction::SetTrue)
                    .help(r"When enabled, hgrep will treat CRLF ('\r\n') as a line terminator instead of just '\n'. This flag is useful on Windows"),
            )
            .arg(
                Arg::new("mmap")
                    .long("mmap")
                    .action(ArgAction::SetTrue)
                    .help("Search using memory maps when possible. mmap is disabled by default unlike ripgrep"),
            )
            .arg(
                Arg::new("max-count")
                    .short('m')
                    .long("max-count")
                    .num_args(1)
                    .value_name("NUM")
                    .help("Limit the number of matching lines per file searched to NUM"),
            )
            .arg(
                Arg::new("max-depth")
                    .long("max-depth")
                    .num_args(1)
                    .value_name("NUM")
                    .help("Limit the depth of directory traversal to NUM levels beyond the paths given"),
            )
            .arg(
                Arg::new("line-regexp")
                    .short('x')
                    .long("line-regexp")
                    .action(ArgAction::SetTrue)
                    .overrides_with("word-regexp")
                    .help("Only show matches surrounded by line boundaries. This is equivalent to putting ^...$ around the search pattern. This flag overrides --word-regexp"),
            )
            .arg(
                Arg::new("pcre2")
                    .short('P')
                    .long("pcre2")
                    .action(ArgAction::SetTrue)
                    .help("When this flag is present, hgrep will use the PCRE2 regex engine instead of its default regex engine"),
            )
            .arg(
                Arg::new("type")
                    .short('t')
                    .long("type")
                    .num_args(1)
                    .value_name("TYPE")
                    .action(clap::ArgAction::Append)
                    .help("Only search files matching TYPE. This option is repeatable. --type-list can print the list of types"),
            )
            .arg(
                Arg::new("type-not")
                    .short('T')
                    .long("type-not")
                    .num_args(1)
                    .value_name("TYPE")
                    .action(clap::ArgAction::Append)
                    .help("Do not search files matching TYPE. Inverse of --type. This option is repeatable. --type-list can print the list of types"),
            )
            .arg(
                Arg::new("type-list")
                    .long("type-list")
                    .action(ArgAction::SetTrue)
                    .help("Show all supported file types and their corresponding globs"),
            )
            .arg(
                Arg::new("max-filesize")
                    .long("max-filesize")
                    .num_args(1)
                    .value_name("NUM+SUFFIX?")
                    .help("Ignore files larger than NUM in size. This does not apply to directories.The input format accepts suffixes of K, M or G which correspond to kilobytes, megabytes and gigabytes, respectively. If no suffix is provided the input is treated as bytes"),
            )
            .arg(
                Arg::new("invert-match")
                    .short('v')
                    .long("invert-match")
                    .action(ArgAction::SetTrue)
                    .help("Invert matching. Show lines that do not match the given pattern"),
            )
            .arg(
                Arg::new("one-file-system")
                    .long("one-file-system")
                    .action(ArgAction::SetTrue)
                    .help("When enabled, the search will not cross file system boundaries relative to where it started from"),
            )
            .arg(
                Arg::new("no-unicode")
                    .long("no-unicode")
                    .action(ArgAction::SetTrue)
                    .help("Disable unicode-aware regular expression matching"),
            )
            .arg(
                Arg::new("regex-size-limit")
                    .long("regex-size-limit")
                    .num_args(1)
                    .value_name("NUM+SUFFIX?")
                    .help("The upper size limit of the compiled regex. The default limit is 10M. For the size suffixes, see --max-filesize"),
            )
            .arg(
                Arg::new("dfa-size-limit")
                    .long("dfa-size-limit")
                    .num_args(1)
                    .value_name("NUM+SUFFIX?")
                    .help("The upper size limit of the regex DFA. The default limit is 10M. For the size suffixes, see --max-filesize"),
            )
            .arg(
                Arg::new("PATTERN")
                    .help("Pattern to search. Regular expression is available"),
            )
            .arg(
                Arg::new("PATH")
                    .help("Paths to search")
                    .num_args(0..)
                    .value_hint(clap::ValueHint::AnyPath)
                    .value_parser(clap::builder::ValueParser::path_buf()),
            );

    cmd
}

fn generate_completion_script<W: io::Write>(shell: &str, out: &mut W) {
    use clap_complete::generate;
    use clap_complete::shells::*;
    use clap_complete_nushell::Nushell;

    let mut cmd = command();
    if shell.eq_ignore_ascii_case("bash") {
        generate(Bash, &mut cmd, "hgrep", out);
    } else if shell.eq_ignore_ascii_case("zsh") {
        generate(Zsh, &mut cmd, "hgrep", out);
    } else if shell.eq_ignore_ascii_case("powershell") {
        generate(PowerShell, &mut cmd, "hgrep", out);
    } else if shell.eq_ignore_ascii_case("fish") {
        generate(Fish, &mut cmd, "hgrep", out);
    } else if shell.eq_ignore_ascii_case("elvish") {
        generate(Elvish, &mut cmd, "hgrep", out);
    } else if shell.eq_ignore_ascii_case("nushell") {
        generate(Nushell, &mut cmd, "hgrep", out);
    } else {
        unreachable!(); // SHELL argument was validated by clap
    }
}

#[cfg(feature = "ripgrep")]
fn build_ripgrep_config(
    min_context: u64,
    max_context: u64,
    matches: &ArgMatches,
) -> Result<ripgrep::Config<'_>> {
    let mut config = ripgrep::Config::default();
    config
        .min_context(min_context)
        .max_context(max_context)
        .no_ignore(matches.get_flag("no-ignore"))
        .hidden(matches.get_flag("hidden"))
        .case_insensitive(matches.get_flag("ignore-case"))
        .smart_case(matches.get_flag("smart-case"))
        .glob_case_insensitive(matches.get_flag("glob-case-insensitive"))
        .pcre2(matches.get_flag("pcre2")) // must be before fixed_string
        .fixed_strings(matches.get_flag("fixed-strings"))
        .word_regexp(matches.get_flag("word-regexp"))
        .follow_symlink(matches.get_flag("follow-symlink"))
        .multiline(matches.get_flag("multiline"))
        .crlf(matches.get_flag("crlf"))
        .multiline_dotall(matches.get_flag("multiline-dotall"))
        .mmap(matches.get_flag("mmap"))
        .line_regexp(matches.get_flag("line-regexp"))
        .invert_match(matches.get_flag("invert-match"))
        .one_file_system(matches.get_flag("one-file-system"))
        .no_unicode(matches.get_flag("no-unicode"));

    if let Some(globs) = matches.get_many::<String>("glob") {
        config.globs(globs.map(String::as_str));
    }

    if let Some(num) = matches.get_one::<String>("max-count") {
        let num = num
            .parse()
            .context("could not parse --max-count option value as unsigned integer")?;
        config.max_count(num);
    }

    if let Some(num) = matches.get_one::<String>("max-depth") {
        let num = num
            .parse()
            .context("could not parse --max-depth option value as unsigned integer")?;
        config.max_depth(num);
    }

    if let Some(size) = matches.get_one::<String>("max-filesize") {
        config
            .max_filesize(size)
            .context("could not parse --max-filesize option value as file size string")?;
    }

    if let Some(limit) = matches.get_one::<String>("regex-size-limit") {
        config
            .regex_size_limit(limit)
            .context("could not parse --regex-size-limit option value as size string")?;
    }

    if let Some(limit) = matches.get_one::<String>("dfa-size-limit") {
        config
            .dfa_size_limit(limit)
            .context("could not parse --dfa-size-limit option value as size string")?;
    }

    let types = matches.get_many::<String>("type");
    if let Some(types) = types {
        config.types(types.map(String::as_str));
    }

    let types_not = matches.get_many::<String>("type-not");
    if let Some(types_not) = types_not {
        config.types_not(types_not.map(String::as_str));
    }

    Ok(config)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum PrinterKind {
    #[cfg(feature = "bat-printer")]
    Bat,
    #[cfg(feature = "syntect-printer")]
    Syntect,
}

fn run(matches: ArgMatches) -> Result<bool> {
    if let Some(shell) = matches.get_one::<String>("generate-completion-script") {
        let stdout = io::stdout();
        generate_completion_script(shell, &mut stdout.lock());
        return Ok(true);
    }

    if matches.get_flag("generate-man-page") {
        let man = clap_mangen::Man::new(command());
        let stdout = io::stdout();
        man.render(&mut stdout.lock())?;
        return Ok(true);
    }

    #[allow(unused_variables)] // printer_kind is unused when syntect-printer is disabled for now
    let printer_kind = match matches.get_one::<String>("printer").unwrap().as_str() {
        #[cfg(feature = "bat-printer")]
        "bat" => PrinterKind::Bat,
        #[cfg(not(feature = "bat-printer"))]
        "bat" => anyhow::bail!("--printer bat is not available because 'bat-printer' feature was disabled at compilation"),
        #[cfg(feature = "syntect-printer")]
        "syntect" => PrinterKind::Syntect,
        #[cfg(not(feature = "syntect-printer"))]
        "syntect" => anyhow::bail!("--printer syntect is not available because 'syntect-printer' feature was disabled at compilation"),
        p => unreachable!(), // Argument paraser already checked this case
    };

    let min_context = matches
        .get_one::<String>("min-context")
        .unwrap()
        .parse()
        .context("could not parse \"min-context\" option value as unsigned integer")?;
    let max_context = matches
        .get_one::<String>("max-context")
        .unwrap()
        .parse()
        .context("could not parse \"max-context\" option value as unsigned integer")?;
    let max_context = cmp::max(min_context, max_context);

    let mut printer_opts = PrinterOptions::default();
    if let Some(width) = matches.get_one::<String>("tab") {
        printer_opts.tab_width = width
            .parse()
            .context("could not parse \"tab\" option value as unsigned integer")?;
    }

    #[cfg(feature = "bat-printer")]
    let theme_env = env::var("BAT_THEME").ok();
    #[cfg(feature = "bat-printer")]
    if printer_kind == PrinterKind::Bat {
        if let Some(var) = &theme_env {
            printer_opts.theme = Some(var);
        }
    }
    if let Some(theme) = matches.get_one::<String>("theme") {
        printer_opts.theme = Some(theme);
    }

    let is_grid = matches.get_flag("grid");
    #[cfg(feature = "bat-printer")]
    if printer_kind == PrinterKind::Bat {
        if let Ok("plain" | "header" | "numbers") =
            env::var("BAT_STYLE").as_ref().map(String::as_str)
        {
            if !is_grid {
                printer_opts.grid = false;
            }
        }
    }
    if matches.get_flag("no-grid") && !is_grid {
        printer_opts.grid = false;
    }

    if let Some(width) = matches.get_one::<String>("term-width") {
        let width = width
            .parse()
            .context("could not parse \"term-width\" option value as unsigned integer")?;
        printer_opts.term_width = width;
        if width < 10 {
            anyhow::bail!("Too small value at --term-width option ({} < 10)", width);
        }
    }

    if let Some(mode) = matches.get_one::<String>("wrap") {
        if mode.eq_ignore_ascii_case("never") {
            printer_opts.text_wrap = TextWrapMode::Never;
        } else if mode.eq_ignore_ascii_case("char") {
            printer_opts.text_wrap = TextWrapMode::Char;
        } else {
            unreachable!(); // Option value was validated by clap
        }
    }

    if matches.get_flag("first-only") {
        printer_opts.first_only = true;
    }

    #[cfg(feature = "syntect-printer")]
    {
        if matches.get_flag("background") {
            printer_opts.background_color = true;
            #[cfg(feature = "bat-printer")]
            if printer_kind == PrinterKind::Bat {
                anyhow::bail!("--background flag is only available for syntect printer since bat does not support painting background colors");
            }
        }

        if matches.get_flag("ascii-lines") {
            printer_opts.ascii_lines = true;
            #[cfg(feature = "bat-printer")]
            if printer_kind == PrinterKind::Bat {
                anyhow::bail!("--ascii-lines flag is only available for syntect printer since bat does not support this feature");
            }
        }
    }

    #[cfg(feature = "bat-printer")]
    if matches.get_flag("custom-assets") {
        printer_opts.custom_assets = true;
        #[cfg(feature = "syntect-printer")]
        if printer_kind == PrinterKind::Syntect {
            anyhow::bail!("--custom-assets flag is only available for bat printer");
        }
    }

    if matches.get_flag("list-themes") {
        #[cfg(feature = "syntect-printer")]
        if printer_kind == PrinterKind::Syntect {
            hgrep::syntect::list_themes(io::stdout().lock(), &printer_opts)?;
            return Ok(true);
        }

        #[cfg(feature = "bat-printer")]
        if printer_kind == PrinterKind::Bat {
            BatPrinter::new(printer_opts).list_themes()?;
            return Ok(true);
        }

        unreachable!();
    }

    #[cfg(feature = "ripgrep")]
    if matches.get_flag("type-list") {
        let config = build_ripgrep_config(min_context, max_context, &matches)?;
        config.print_types(io::stdout().lock())?;
        return Ok(true);
    }

    #[cfg(feature = "ripgrep")]
    if let Some(pattern) = matches.get_one::<String>("PATTERN") {
        use std::path::PathBuf;

        let paths = matches
            .get_many::<PathBuf>("PATH")
            .map(|p| p.map(PathBuf::as_path));
        let config = build_ripgrep_config(min_context, max_context, &matches)?;

        #[cfg(feature = "syntect-printer")]
        if printer_kind == PrinterKind::Syntect {
            let printer = SyntectPrinter::with_stdout(printer_opts)?;
            return ripgrep::grep(printer, pattern, paths, config);
        }

        #[cfg(feature = "bat-printer")]
        if printer_kind == PrinterKind::Bat {
            let printer = std::sync::Mutex::new(BatPrinter::new(printer_opts));
            return ripgrep::grep(printer, pattern, paths, config);
        }

        unreachable!();
    }

    #[cfg(feature = "syntect-printer")]
    if printer_kind == PrinterKind::Syntect {
        use hgrep::printer::Printer;
        use rayon::prelude::*;
        let printer = SyntectPrinter::with_stdout(printer_opts)?;
        return io::BufReader::new(io::stdin())
            .grep_lines()
            .chunks_per_file(min_context, max_context)
            .par_bridge()
            .map(|file| {
                printer.print(file?)?;
                Ok(true)
            })
            .try_reduce(|| false, |a, b| Ok(a || b));
    }

    #[cfg(feature = "bat-printer")]
    if printer_kind == PrinterKind::Bat {
        let mut found = false;
        let printer = BatPrinter::new(printer_opts);
        let stdin = io::stdin();
        for f in io::BufReader::new(stdin.lock())
            .grep_lines()
            .chunks_per_file(min_context, max_context)
        {
            printer.print(f?)?;
            found = true;
        }
        return Ok(found);
    }

    unreachable!();
}

fn main() {
    #[cfg(windows)]
    if let Err(code) = nu_ansi_term::enable_ansi_support() {
        eprintln!("ANSI color support could not be enabled with error code {code}");
        process::exit(2);
    }

    let status = match Args::new().and_then(|a| run(command().get_matches_from(a))) {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(err) => {
            eprintln!("\x1b[1;91merror:\x1b[0m {}", err);
            for err in err.chain().skip(1) {
                eprintln!("  Caused by: {}", err);
            }
            2
        }
    };

    process::exit(status);
}

#[cfg(test)]
mod tests {
    use super::*;

    const EMPTY: [OsString; 0] = [];
    #[cfg(not(windows))]
    const SNAPSHOT_DIR: &str = "../testdata/snapshots";
    #[cfg(windows)]
    const SNAPSHOT_DIR: &str = r#"..\testdata\snapshots"#;

    mod arg_matches {
        use super::*;

        fn get_raw_matched_arguments(mat: &ArgMatches) -> Vec<(String, Vec<String>)> {
            let mut v = mat
                .ids()
                .map(|id| {
                    let id = id.as_str().to_string();
                    let args = mat
                        .get_raw(&id)
                        .map(|values| values.map(|v| v.to_string_lossy().to_string()).collect())
                        .unwrap_or_default();
                    (id, args)
                })
                .collect::<Vec<_>>();
            v.sort();
            v
        }

        macro_rules! snapshot_test {
            ($name:ident, $args:expr) => {
                #[test]
                fn $name() {
                    let mut settings = insta::Settings::clone_current();
                    settings.set_snapshot_path(SNAPSHOT_DIR);
                    settings.bind(|| {
                        let cmd = command();
                        let mat = cmd.try_get_matches_from($args).unwrap();
                        let raw = get_raw_matched_arguments(&mat);
                        insta::assert_debug_snapshot!(raw);
                    });
                }
            };
        }

        snapshot_test!(no_arg, EMPTY);
        snapshot_test!(pat_only, ["pat"]);
        snapshot_test!(pat_and_dir, ["pat", "dir1"]);
        snapshot_test!(pat_and_dirs, ["pat", "dir1", "dir2", "dir3"]);
        snapshot_test!(min_max_long, ["--min-context", "2", "--max-context", "4"]);
        snapshot_test!(min_max_short, ["-c", "2", "-C", "4"]);
        snapshot_test!(grid, ["--grid"]);
        snapshot_test!(no_grid, ["--no-grid"]);
        snapshot_test!(theme, ["--theme", "Nord"]);
        snapshot_test!(tab, ["--tab", "8"]);
        snapshot_test!(bat_printer_long, ["--printer", "bat"]);
        snapshot_test!(bat_printer_short, ["-p", "bat"]);
        snapshot_test!(term_width, ["--term-width", "200"]);
        snapshot_test!(wrap_mode, ["--wrap", "never"]);
        snapshot_test!(first_only, ["--first-only"]);
        snapshot_test!(background, ["--background"]);
        snapshot_test!(ascii_lines, ["--ascii-lines"]);
        snapshot_test!(custom_assets, ["--printer", "bat", "--custom-assets"]);
        snapshot_test!(list_themes, ["--list-themes"]);
        snapshot_test!(type_list, ["--type-list"]);
        snapshot_test!(
            generate_completion_script,
            ["--generate-completion-script", "bash"]
        );
        snapshot_test!(generate_man_page, ["--generate-man-page"]);
        snapshot_test!(max_filesize, ["--max-filesize", "100M"]);
        snapshot_test!(
            all_printer_opts_before_args,
            [
                "--min-context",
                "5",
                "--max-context",
                "10",
                "--grid",
                "--no-grid",
                "--theme",
                "Nord",
                "--tab",
                "2",
                "--printer",
                "syntect",
                "--term-width",
                "120",
                "--wrap",
                "never",
                "--first-only",
                "--background",
                "--ascii-lines",
                "--custom-assets",
                "--list-themes",
                "some pattern",
                "dir1",
                "dir2",
            ]
        );
        snapshot_test!(
            all_printer_opts_after_args,
            [
                "some pattern",
                "dir1",
                "dir2",
                "--min-context",
                "5",
                "--max-context",
                "10",
                "--grid",
                "--no-grid",
                "--theme",
                "Nord",
                "--tab",
                "2",
                "--printer",
                "syntect",
                "--term-width",
                "120",
                "--wrap",
                "never",
                "--first-only",
                "--background",
                "--ascii-lines",
                "--custom-assets",
                "--list-themes",
            ]
        );

        #[test]
        fn invalid_option() {
            for args in [
                &["--min-context", "foo"][..],
                &["--max-context", "foo"][..],
                &["--term-width", "foo"][..],
                &["--term-width", "1"][..],
                &["--tab", "foo"][..],
                &["--printer", "syntect", "--custom-assets"][..],
                &["--printer", "bat", "--background"][..],
                &["--printer", "bat", "--ascii-lines"][..],
            ] {
                let mat = command().try_get_matches_from(args).unwrap();
                assert!(run(mat).is_err(), "args: {:?}", args);
            }
        }

        #[test]
        fn arg_parser_debug_assert() {
            command().debug_assert();
        }

        #[test]
        fn arg_parse_error() {
            for args in [
                &["--unknown-arg"][..],
                &["--printer", "foo"][..],
                &["--wrap", "foo"][..],
                &["--generate-completion-script", "unknown-shell"][..],
            ] {
                let parsed = command().try_get_matches_from(args);
                assert!(parsed.is_err(), "args: {:?}", args);
            }
        }
    }

    mod ripgrep_config {
        use super::*;

        macro_rules! snapshot_test {
            ($name:ident, $args:expr) => {
                #[test]
                fn $name() {
                    let mut settings = insta::Settings::clone_current();
                    settings.set_snapshot_path(SNAPSHOT_DIR);
                    settings.bind(|| {
                        let mat = command().try_get_matches_from($args).unwrap();
                        let min_ctx = mat
                            .get_one::<String>("min-context")
                            .unwrap()
                            .parse()
                            .unwrap();
                        let max_ctx = mat
                            .get_one::<String>("max-context")
                            .unwrap()
                            .parse()
                            .unwrap();

                        let cfg = build_ripgrep_config(min_ctx, max_ctx, &mat).unwrap();
                        insta::assert_debug_snapshot!(cfg);
                    });
                }
            };
        }

        snapshot_test!(no_arg, EMPTY);
        snapshot_test!(pat_only, ["pat"]);
        snapshot_test!(pat_and_dirs, ["pat", "dir1", "dir2"]);
        snapshot_test!(glob_one, ["--glob", "*.txt", "pat", "dir"]);
        snapshot_test!(
            glob_many,
            ["-g", "*.txt", "-g", "*.rs", "-g", "*.md", "pat", "dir"]
        );
        snapshot_test!(glob_before_opt, ["-g", "*.txt", "-i", "pat", "dir"]);
        snapshot_test!(glob_arg_with_hyphen, ["-g", "-foo_*.txt", "pat", "dir"]);
        snapshot_test!(ignore_case_smart_case, ["-i", "-S", "pat", "dir"]);
        snapshot_test!(smart_case_ignore_case, ["-S", "-i", "pat", "dir"]);
        snapshot_test!(max_count, ["--max-count", "100", "pat", "dir"]);
        snapshot_test!(max_count_short, ["-m", "100", "pat", "dir"]);
        snapshot_test!(max_depth, ["--max-depth", "10", "pat", "dir"]);
        snapshot_test!(line_regexp_word_regexp, ["-x", "-w", "pat", "dir"]);
        snapshot_test!(word_regexp_line_regexp, ["-w", "-x", "pat", "dir"]);
        snapshot_test!(pcre2, ["-P", "pat", "dir"]);
        snapshot_test!(fixed_string_override_pcre2, ["-F", "-P", "pat", "dir"]);
        snapshot_test!(type_one, ["--type", "rust", "pat", "dir"]);
        snapshot_test!(type_many, ["-t", "rust", "-t", "go", "pat", "dir"]);
        snapshot_test!(type_not_one, ["--type-not", "rust", "pat", "dir"]);
        snapshot_test!(type_not_many, ["-T", "rust", "-T", "go", "pat", "dir"]);
        snapshot_test!(
            type_and_type_not_many,
            ["-t", "rust", "-T", "rust", "-T", "go", "-t", "go", "pat", "dir"]
        );
        snapshot_test!(
            regex_size_limit,
            ["--regex-size-limit", "20M", "pat", "dir"]
        );
        snapshot_test!(dfa_size_limit, ["--dfa-size-limit", "20M", "pat", "dir"]);
        snapshot_test!(
            bool_long_flags,
            [
                "--no-ignore",
                "--ignore-case",
                "--smart-case",
                "--glob-case-insensitive",
                "--fixed-strings",
                "--word-regexp",
                "--follow",
                "--multiline",
                "--multiline-dotall",
                "--crlf",
                "--mmap",
                "--hidden",
                "--line-regexp",
                "--pcre2",
                "--one-file-system",
                "--no-unicode",
                "pat",
                "dir",
            ]
        );
        snapshot_test!(
            bool_short_flags,
            ["-i", "-S", "-F", "-w", "-L", "-U", "-.", "-x", "-P", "pat", "dir"]
        );
        snapshot_test!(max_filesize, ["--max-filesize", "100M"]);
    }

    #[test]
    fn generate_completion() {
        for shell in COMPLETION_SHELLS {
            let mut v = vec![];
            generate_completion_script(shell, &mut v);
            assert!(!v.is_empty(), "shell: {}", shell);
        }
    }

    mod args {
        use super::*;
        use std::ffi::OsString;
        use std::sync::Mutex;

        struct Guard {
            saved: Option<String>,
        }
        impl Guard {
            fn new() -> Self {
                Self {
                    saved: env::var(OPTS_ENV_VAR).ok(),
                }
            }
        }
        impl Drop for Guard {
            fn drop(&mut self) {
                if let Some(v) = &self.saved {
                    env::set_var(OPTS_ENV_VAR, v);
                } else {
                    env::remove_var(OPTS_ENV_VAR);
                }
            }
        }

        static MU: Mutex<()> = Mutex::new(());

        #[test]
        fn iterate_args() {
            let _lock = MU.lock().unwrap();
            let _guard = Guard::new();

            for (env, prefix) in [
                ("-i", &["-i"][..]),
                ("-i -S", &["-i", "-S"][..]),
                ("'-i'", &["-i"][..]),
                ("'foo bar'", &["foo bar"][..]),
                (r#""foo\\ bar""#, &[r#"foo\ bar"#][..]),
                ("", &[][..]),
            ] {
                env::set_var(OPTS_ENV_VAR, env);

                let have = Args::new().unwrap().collect::<Vec<_>>();
                let mut want = prefix.iter().map(OsString::from).collect::<Vec<_>>();
                let mut args = env::args_os();
                args.next(); // Omit the executable name at the first argument
                want.extend(args);

                assert_eq!(want, have, "{env:?}, {prefix:?}");
            }
        }

        #[test]
        fn no_env_for_args() {
            let _lock = MU.lock().unwrap();
            let _guard = Guard::new();
            env::remove_var(OPTS_ENV_VAR);

            let have = Args::new().unwrap().collect::<Vec<_>>();
            let mut want = env::args_os().collect::<Vec<_>>();
            want.remove(0);
            assert_eq!(want, have);
        }

        #[test]
        fn broken_shell_command_in_env() {
            let _lock = MU.lock().unwrap();
            let _guard = Guard::new();
            env::set_var(OPTS_ENV_VAR, "'-i");

            let err = Args::new().unwrap_err();
            let msg = format!("{}", err);
            assert!(
                msg.contains("cannot be parsed as a shell command"),
                "{msg:?}",
            );
        }

        #[test]
        #[cfg(not(windows))]
        fn invalid_utf8_sequence_in_env() {
            use std::ffi::OsStr;
            use std::os::unix::ffi::OsStrExt;

            let _lock = MU.lock().unwrap();
            let _guard = Guard::new();
            env::set_var(OPTS_ENV_VAR, OsStr::from_bytes(b"\xc3\x28"));

            let err = Args::new().unwrap_err();
            let msg = format!("{}", err);
            assert!(msg.contains("is not a valid UTF-8 sequence"), "{msg:?}");
        }
    }
}
