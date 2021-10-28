hgrep: Human-friendly GREP
==========================
[![CI][ci-badge]][ci]
[![crate][crates-io-badge]][crates-io]

[hgrep][] is a grep tool to search files with a given pattern and print the matched code snippets with human-friendly syntax
highlighting. This tool brings search results like the code search on GitHub to your local machine. In short, it's something like
searching files with [ripgrep][] and showing results with [bat][].

This is similar to `-C` option of `grep` command. hgrep is useful to survey the matches with contexts around them. When some
matches are near enough, hgrep prints the lines within one code snippet. Unlike `grep -C`, hgrep adopts some heuristics around
blank lines to determine an efficient number of context lines.

<img src="https://github.com/rhysd/ss/raw/master/hgrep/main.png" alt="screenshot" width="766" height="739" />

Example:

```sh
# With standard grep
grep -nH pattern -R ./dir | hgrep

# With grep alternative tools
rg -nH pattern ./dir | hgrep
```

As an optional feature, hgrep has built-in grep implementation thanks to [ripgrep][] as a library. It's a subset of `rg` command.
And it's faster when there are so many matches since everything is done in the same process.

Example:

```sh
# Use built-in subset of ripgrep
hgrep pattern ./dir
```

hgrep provides two printers to print match results for your use case. Please see ['`bat` printer v.s. `syntect` printer'][bat-vs-syntect]
section for the comparison.

- `syntect` printer: Our own implementation of printer using [syntect][] library. Performance and its output layout are more optimized
- `bat` printer: Printer built on top of [bat][]'s pretty printer implementation, which is battle-tested and provides some unique features

Please see [the usage section](#usage) for more details.

## Installation

### Binary releases

Visit [the releases page][releases] and download the zip file for your platform. Unarchive the file and put the executable file
in some `$PATH` directory. Currently, the following targets are supported. If you want a binary for some other platform, feel free
to make an issue to request it.

- Linux (x86_64)
- macOS (x86_64, aarch64)
- Windows (x86_64)

### Via [Homebrew][homebrew]

By adding hgrep repository as Homebrew tap, `hgrep` command can be installed and managed via Homebrew. Currently, only for x86_64
macOS and Linux.

```sh
brew tap "rhysd/hgrep" "https://github.com/rhysd/hgrep"
brew install hgrep
```

### Via [MacPorts][macports]

On macOS, you can install `hgrep` with the following commands through MacPorts:

```sh
sudo port selfupdate
sudo port install hgrep
```

### For NetBSD

To install pre-built binaries using the package manager, simply run:

```sh
pkgin install hgrep
```

Or, if you prefer to build from source,

```sh
cd /usr/pkgsrc/textproc/hgrep
make install
```

### Via [cargo][] package manager

```sh
cargo install hgrep
```

If you always use hgrep with reading the grep output from stdin and don't want the built-in ripgrep feature, it can be omitted.
This reduces the number of dependencies, installation time, and binary size.

```sh
cargo install hgrep --no-default-features --features bat-printer,syntect-printer
```

To customize features on installation, please see the following 'Feature flags' section for more details.

### Feature flags

All features are optional and enabled by default. At least `bat-printer` or `syntect-printer` needs to be enabled.

| Feature           | Description                                                                                                                   |
|-------------------|-------------------------------------------------------------------------------------------------------------------------------|
| `ripgrep`         | Built-in grep implementation built on top of [ripgrep][] as a library. Performance is better than piping `rg` in some cases.  |
| `bat-printer`     | Printer implementation built on top of [bat][]'s pretty printer, which is battle-tested and provides some unique features.    |
| `syntect-printer` | Our own printer implementation built with [syntect][] library. Performance and output layout are optimized for our use cases. |

For the differences of `bat-printer` and `syntect-printer`, see ['`bat` printer v.s. `syntect` printer'][bat-vs-syntect] section.

## Usage

### Eat `grep -nH` output

hgrep takes grep results via stdin. Since hgrep expects file paths and line numbers in each line of the output, `-nH` is
necessary at `grep` command.

```sh
grep -nH pattern -R paths... | hgrep [options...]
```

`grep` alternative tools like [ripgrep][], [ag][], [pt][], ... are also available because they can output results compatible with
`grep -nH`.

```sh
rg -nH pattern paths... | hgrep [options...]
```

When you want a pager, please use external commands like `less`. `$COLUMNS` needs to be passed because terminal width is fixed to
80 characters when the process is piped. If you frequently use a pager, ['Set default command options'](#set-default-command-options)
section would describe a better way.

```sh
grep -nH pattern -R paths... | hgrep --term-width "$COLUMNS" [options...] | less -R
```

By default, hgrep shows at least 5 lines and at most 5 lines as context of a match. How many context lines is determined by some
heuristics around blank lines for efficiency. Minimum context lines can be specified by `-c` and Maximum context lines can be
specified by `-C`. If you don't want the heuristics, specify the same value to the options like `-c 10 -C 10`.

```sh
# At least 10 context lines and at most 20 context lines
grep -nH pattern -R paths... | hgrep -c 10 -C 20
```

### Built-in ripgrep

Optionally hgrep provides built-in grep implementation. It is a subset of ripgrep since it's built using ripgrep as a library.
It's faster when there are so many matches because everything is done in the same process. In combination with `syntect-printer`
feature, matched regions can be highilghted in a searched text color.

The built-in grep feature is enabled by default and can be omitted by feature flags.

```sh
hgrep [options...] pattern paths...
```

Though almost all useful options are implemented, the built-in grep implementation is a subset of ripgrep. If you need full
functionalities, use `rg` command and eat its output by hgrep via stdin. Currently there are the following restrictions.

- Preprocessor is not supported (e.g. search zip files)
- Memory map is not used until `--mmap` flag is specified
- Adding and removing file types are not supported. Only default file types are supported (see `--type-list`)
- `.ripgreprc` config file is not supported

### `bat` printer v.s. `syntect` printer

hgrep provides two printers to print match results; `bat` printer and `syntect` printer. `bat` printer is a printer
implementation built on top of [bat][]'s pretty printer. And `syntect` printer is our own printer implementation built with
[syntect][] library. `--printer` (or `-p`) flag can specify the printer to print results.

At first, there was `bat` printer only. And then `syntect` printer was implemented for better performance and optimized layout.

#### Pros of each printer

- `syntect` printer
  - Performance is much better. 2x to 4x faster (more match results get better performance)
  - Output layout is optimized for our use cases. Matched regions are highlighted in a searched text color. A line number at a
    match is highlighted in a different color.
  - Painting background color (`--background`) is supported. This is useful when your favorite theme does not fit to your
    terminal's background color
  - Compatibility for old terminals is better. It automatically changes the default theme to 'ansi' for 16-colors terminals. And
    it provides `--ascii-lines` flag to draw border lines with ascii characters instead of Unicode characters like '├', '┬', and
    so on
- `bat` printer
  - Implementation is battle-tested. It is already used by many users on many platforms and terminals
  - The behavior is compatible with `bat` command. Its output layout is the same as `bat` command. It can load bat's assets cache

Currently, `bat` is the default painter (unless `bat-printer` feature is disabled) because the implementation is not mature yet.
But in 0.2 release, changing the default painter to `syntect` is planned.

#### Why performance of `syntect` printer is better?

Syntax highlighting is very CPU-heavy task. Many regular expression matchings happen at each line. For accurate syntax
highlighting, a highlighter needs to parse the syntax at the beginning of the file. It means that printing a match at the last
line of a file is a much heavier task than printing a match of the first line of the file.

Since `syntect` printer is designed for calculating syntax highlights per file in parallel, its performance is much better. It's
2x~4x faster than `bat` printer in some experiments. More match results get better performance.

In contrast, bat is not designed for multi-threads. It's not possible to share `bat::PrettyPrinter` instance among threads. It
means that printing match results including syntax highlighting must be done in a single thread.

| `syntect` printer sequence | `bat` printer sequence |
|----------------------------|------------------------|
| ![](https://github.com/rhysd/ss/raw/master/hgrep/comparison_syntect.png) | ![](https://github.com/rhysd/ss/raw/master/hgrep/comparison_bat.png) |

### Change color theme and layout

The default color theme is `Monokai Extended` respecting `bat` command's default. Other theme can be specified via `--theme`
option. To know names of themes, try `--list-themes` flag.

```sh
grep -nH ... | hgrep --theme Nord
```

And hgrep respects `BAT_THEME` environment variable.

```sh
export BAT_THEME=OneHalfDark
```

The default layout is 'grid' respecting `bat` command's default. To print the matches without borderlines, `--no-grid` option
is available.

```sh
grep -nH ... | hgrep --no-grid
```

And hgrep respects `BAT_STYLE` environment variable. When `plain` or `header` or `numbers` is set, hgrep removes borderlines.

```sh
export BAT_STYLE=numbers
```

### Set default command options

Wrapping `hgrep` command with shell's `alias` command works fine for setting default command options.

For example, if you're using Bash, put the following line in your `.bash_profile`.

```sh
# Use syntect-printer and search hidden files by default
alias hgrep='hgrep --printer syntect --hidden'
```

If you prefer a pager, try the following wrapper function. `--term-width` propagates the correct width of the terminal window.

```sh
# Use syntect-printer and less as pager. $COLUMNS corrects terminal window
function hgrep() {
    command hgrep -p syntect --term-width "$COLUMNS" "$@" | less -R
}
```

### Command options

- Common options
  - `--min-context NUM` (`-c`): Minimum lines of leading and trailing context surrounding each match. Default value is 5
  - `--max-context NUM` (`-C`): Maximum lines of leading and trailing context surrounding each match. Default value is 10
  - `--no-grid` (`-G`): Remove borderlines for more compact output. `--grid` flag is an opposite of this flag
  - `--tab NUM`: Number of spaces for tab character. Set 0 to pass tabs through. Default value is 4
  - `--theme THEME`: Theme for syntax highlighting. Default value is the same as `bat` command
  - `--list-themes`: List all theme names available for --theme option
  - `--printer`: Printer to print the match results. 'bat' or 'syntect' is available. Default value is 'bat'
  - `--term-width`: Width (number of characters) of terminal window
  - `--first-only` (`-f`): Show only the first code snippet per file
- Only for `ripgrep` feature
  - `--no-ignore`: Don't respect ignore files (.gitignore, .ignore, etc.)
  - `--ignore-case` (`-i`): When this flag is provided, the given patterns will be searched case insensitively
  - `--smart-case` (`-S`): Search case insensitively if the pattern is all lowercase. Search case sensitively otherwise
  - `--glob GLOB...` (`-g`): Include or exclude files and directories for searching that match the given glob
  - `--glob-case-insensitive`: Process glob patterns given with the -g/--glob flag case insensitively
  - `--fixed-strings` (`-F`): Treat the pattern as a literal string instead of a regular expression
  - `--word-regexp` (`-w`): Only show matches surrounded by word boundaries
  - `--follow` (`-L`): When this flag is enabled, hgrep will follow symbolic links while traversing directories
  - `--multiline` (`-U`): Enable matching across multiple lines
  - `--multiline-dotall`: Enable "dot all" in your regex pattern, which causes '.' to match newlines when multiline searching is enabled
  - `--crlf`: about(r"When enabled, hgrep will treat CRLF (`\r\n`) as a line terminator instead of just `\n`. This flag is useful on Windows
  - `--mmap`: Search using memory maps when possible. mmap is disabled by default unlike hgrep
  - `--max-count NUM` (`-m`): Limit the number of matching lines per file searched to NUM
  - `--max-depth NUM`: Limit the depth of directory traversal to NUM levels beyond the paths given
  - `--max-filesize NUM`: Ignore files larger than NUM in size
  - `--line-regexp` (`-x`): Only show matches surrounded by line boundaries. This is equivalent to putting ^...$ around all of the search patterns
  - `--pcre2` (`-P`): When this flag is present, hgrep will use the PCRE2 regex engine instead of its default regex engine
  - `--type TYPE` (`-t`): Only search files matching TYPE. This option is repeatable
  - `--type-not TYPE` (`-T`): Do not search files matching TYPE. Inverse of --type. This option is repeatable
  - `--type-list`: Show all supported file types and their corresponding globs
  - `--wrap MODE`: Text-wrapping mode. 'char' enables character-wise text-wrapping. 'never' disables text-wrapping. Default value is 'char'
- Only for `syntect-printer` feature
  - `--background`: Paint background colors. This is useful when your favorite theme does not fit to your terminal's background color
  - `--ascii-lines`: Use ascii characters for drawing border lines instead of Unicode characters
- Only for `bat-printer` feature
  - `--custom-assets`: Load bat's custom assets from cache. Note that this flag may not work with some version of `bat` command

See `--help` for the full list of available options in your environment.

### Generate completion scripts

Shell completion script for `hgrep` command is available. `--generate-completion-script` option generates completion script and
prints it to stdout. [Bash][bash], [Zsh][zsh], [Fish][fish], [PowerShell][pwsh], [Elvish][elvish] are supported. See `--help` for
the argument of the option.

This is an example of setup the completion script on Zsh.

```sh
# Let's say we set comps=~/.zsh/site-functions
hgrep --generate-completion-script zsh > ~/.zsh/site-functions/_hgrep
```

## Alternatives

Some other alternatives instead of using hgrep.

### Small ShellScript to combine `ripgrep` and `bat`

ripgrep and bat are well-designed tools so they can be used as building parts of a small script.

```sh
rg -nH ... | while IFS= read -r line; do
  # Parse $line and calculate the range of snippet and highlighted lines
  file=...
  lines=...
  range=...

  # Show matched snippet
  bat -H ${lines} -r ${range} ${file}
done
```

It works fine but hgrep is more optimized for this usage.

- When the matches are near enough, the lines are printed in one snippet.
- Performance is better than running `bat` process per matched line.
- hgrep computes efficient context lines based on some heuristics.
- hgrep is available where ShellScript is unavailable (e.g. PowerShell).

### Fuzzy finder like `fzf` with `bat` preview window

Fuzzy finder like [fzf][] provides a preview window functionality and `bat` can print the match in the preview window.

```sh
grep -nH ... | \
    fzf --preview='bat --pager never --color always -H {2} -r {2}: -p {1}' --delimiter=:
```

This usage is great when you need the incremental search, but you need to check each preview of matches one by one.

hgrep focuses on surveying all the matches.

## Bug reporting

Please [make an issue on GitHub][new-issue]. Ensure to describe how to reproduce the bug.

## License

hgrep is distributed under [the MIT license](./LICENSE.txt).

[hgrep]: https://github.com/rhysd/hgrep
[ripgrep]: https://github.com/BurntSushi/ripgrep
[bat]: https://github.com/sharkdp/bat
[cargo]: https://github.com/rust-lang/cargo
[ag]: https://github.com/ggreer/the_silver_searcher
[pt]: https://github.com/monochromegane/the_platinum_searcher
[fzf]: https://github.com/junegunn/fzf
[ci-badge]: https://github.com/rhysd/hgrep/actions/workflows/ci.yml/badge.svg
[ci]: https://github.com/rhysd/hgrep/actions/workflows/ci.yml
[crates-io]: https://crates.io/crates/hgrep
[crates-io-badge]: https://img.shields.io/crates/v/hgrep.svg
[releases]: https://github.com/rhysd/hgrep/releases
[bash]: https://www.gnu.org/software/bash/
[zsh]: https://www.zsh.org/
[fish]: https://fishshell.com/
[pwsh]: https://docs.microsoft.com/en-us/powershell/
[elvish]: https://elv.sh/
[homebrew]: https://brew.sh/
[macports]: https://www.macports.org/
[new-issue]: https://github.com/rhysd/hgrep/issues/new
[syntect]: https://github.com/trishume/syntect
[bat-vs-syntect]: #bat-printer-vs-syntect-printer
