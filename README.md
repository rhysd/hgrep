hgrep: Human-friendly GREP
==========================
[![CI][ci-badge]][ci]
[![crate][crates-io-badge]][crates-io]

[hgrep][] is a grep tool to search files with given pattern and print the matched code snippets with human-friendly syntax
highlighting. In short, it's a fusion of [bat][] and grep or other alternatives like [ripgrep][].

This is similar to `-C` option of `grep` command, but hgrep focuses on human readable outputs. hgrep is useful to survey the
matches with contexts around them. When some matches are near enough, hgrep prints the lines within one code snippet. Unlike
`grep -C`, hgrep adopts some heuristics around blank lines to determine efficient number of context lines.

Example:

```sh
# With standard grep
grep -nH pattern -R ./dir | hgrep

# With grep alternative tools
rg -nH pattern ./dir | hgrep
```

As an optional feature, hgrep has builtin grep implementation thanks to ripgrep as library. It's a subset of `rg` command. And
it's faster when there are so many matches since everything is done in the same process.

Example:

```sh
# Use builtin subset of ripgrep
hgrep pattern ./dir
```

Please see [the usage section](#usage) for more details.

<img src="https://github.com/rhysd/ss/raw/master/hgrep/main.png" alt="screenshot" width="986" height="734" />

## Installation

### Releases

Visit [the releases page][releases] and download the zip file for your platform. Unarchive the file and put the executable file
in some `$PATH` directory. Currently x86_64 Linux/macOS/Windows binaries are supported.

### Via [cargo][] package manager

```sh
cargo install hgrep
```

If you always use hgrep with reading the grep output from stdin and don't want the builtin ripgrep feature, it can be omitted.
This reduces the number of dependencies, installation time, and the binary size.

```sh
cargo install hgrep --no-default-features
```

## Usage

### Eat `grep -nH` output

hgrep takes grep results via stdin. Since hgrep expects file paths and line numbers in each line of the output, `-nH` is
necessary at `grep` command.

```sh
grep -nH pattern -R paths... | hgrep
```

`grep` alternative tools like [ripgrep][], [ag][], [pt][], ... are also available because they can output results compatible with
`grep -nH`.

```sh
rg -nH pattern paths... | hgrep
```

When you want a pager, please use external commands like `less`.

```sh
grep -nH pattern -R paths... | hgrep | less -R
```

By default, hgrep shows at least 5 lines and at most 5 lines as context of a match. How many context lines is determined by some
heuristics around blank lines for efficiency. Minimum context lines can be specified by `-c` and Maximum context lines can be
specified by `-C`. If you don't want the heuristics, specify the same value to the options like `-c 10 -C 10`.

```sh
# At least 10 context lines and at most 20 context lines
grep -nH pattern -R paths... | hgrep -c 10 -C 20
```

### Builtin ripgrep

Optionally hgrep provides builtin grep implementation. It is a subset of ripgrep since it's built using ripgrep as library. And
it's faster when there are so many matches because everything is done in the same process. The builtin grep feature is enabled by
default and can be omitted by installing it with `--no-default-features`.

```sh
hgrep pattern paths...
```

Since it is a subset, there are some restrictions against ripgrep. If you need full functionalities, use `rg` command and eat its
output by hgrep.

- Some functionalities (e.g. preprocessor) are not supported
- Memory map is not used until `--mmap` flag is specified
- Context option is `-c`, not `-C`, and set to 10 by default
- Adding/Removing file types are not supported. Only default file types are supported (see `--type-list`)
- `.ripgreprc` config file is not supported

### Change color theme and layout

Default color theme is `Monokai Extended` respecting `bat` command's default. Other theme can be specified via `--theme` option.

```sh
grep -nH ... | hgrep --theme Nord
```

And hgrep respects `BAT_THEME` environment variable.

```sh
export BAT_THEME=OneHalfDark
```

Default layout is 'grid' respecting `bat` command's default. To print the matches without border lines, `--no-grid` option is
available.

```sh
grep -nH ... | hgrep --no-grid
```

And hgrep respects `BAT_STYLE` environment variable. When `plain` or `header` or `numbers` is set, hgrep removes border lines.

```sh
export BAT_STYLE=numbers
```

### Command options

- Common options
  - `--min-context NUM` (`-c`): Minimum lines of leading and trailing context surrounding each match. Default value is 5
  - `--max-context NUM` (`-C`): Maximum lines of leading and trailing context surrounding each match. Default value is 5
  - `--no-grid` (`-G`): Remove border lines for more compact output
  - `--tab NUM`: Number of spaces for tab character. Set 0 to pass tabs through. Default value is 4
  - `--theme THEME`: Theme for syntax highlighting. Default value is the same as `bat` command
  - `--list-themes`: List all theme names available for --theme option
- Only for builtin ripgrep
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

See `--help` for full list of options.

### Generate completion scripts

Shell completion script for `hgrep` command is available. `--generate-completion-script` option generates completion script and
prints it to stdout. [Bash][bash], [Zsh][zsh], [Fish][fish], [PowerShell][pwsh], [Elvish][elvish] are supported. See `--help` for
argument of the option.

This is example to setup the script completion script on Zsh.

```sh
# Let's say we set comps=~/.zsh/site-functions
hgrep --generate-completion-script zsh > ~/.zsh/site-functions/_hgrep
```

## Alternatives

Some other alternatives instead of using hgrep.

### Small ShellScript to combine `ripgrep` and `bat`

ripgrep and bat are well-designed tools so they can be used as building parts of small script.

```sh
grep -nH ... | while IFS= read -r line; do
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

Fuzzy finder like [fzf][] privides a preview window functionality and `bat` can print the match previews.

```sh
grep -nH ... | \
    fzf --preview='bat --pager never --color always -H {2} -r {2}: -p {1}' --delimiter=:
```

This usage is great when you need the incremental search, but you need to check each preview of matches one by one.

hgrep focuses on surveying all the matches.

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
