bat + grep = batgrep
====================

[batgrep][] is a grep tool to search files with given pattern and print the matched code snippets with syntax highlighting.
In short, it's a fusion of [bat][] and grep or other alternatives like [ripgrep][].

This is similar to `-C` option of `grep` command, but its output is enhanced with syntax highlighting. It focuses on human
readable outputs. batgrep is useful to survey the matches with contexts around them. When some matches are near enough, batgrep
prints the lines within one code snippet.

batgrep takes matched results via stdin and prints them with syntax highlighted snippets. Ensure to give `-n -H` option to the
`grep` command.

Example:

```sh
# With standard grep
grep -nH pattern -R ./dir | batgrep

# With grep alternative tools
rg -nH pattern ./dir | batgrep
```

As an optional feature, batgrep has builtin grep implementation thanks to ripgrep as library. It's a subset of `rg` command. And
it's faster when there are so many matches since everything is done in the same process.

Example:

```sh
# Use builtin subset of ripgrep
batgrep pattern ./dir
```

Please see [the usage section](#usage) for more details.

## Installation

Via [cargo][] package manager, which is included in Rust toolchain.

```sh
cargo install batgrep
```

If you always use batgrep with reading the grep output from stdin and don't want the builtin ripgrep feature, it can be omitted.
This reduces the number of dependencies, installation time, and the binary size.

```sh
cargo install batgrep --no-default-features
```

## Usage

### Eat `grep -nH` output

batgrep takes grep results via stdin. Since batgrep expects file paths and line numbers in each line of the output, `-nH` is
necessary at `grep` command.

```sh
grep -nH pattern -R paths... | batgrep
```

`grep` alternative tools like [ripgrep][], [ag][], [pt][], ... are also available because they can output results compatible with
`grep -nH`.

```sh
rg -nH pattern paths... | batgrep
```

When you want a pager, please use external commands like `less`.

```sh
grep -nH pattern -R paths... | batgrep | less -R
```

### Builtin ripgrep

Optionally batgrep provides builtin grep implementation. It is a subset of ripgrep since it's built using ripgrep as library. And
it's faster when there are so many matches because everything is done in the same process. The builtin grep feature is enabled by
default and can be omitted by installing it with `--no-default-features`.

```sh
batgrep pattern paths...
```

Since it is a subset, there are some restrictions against ripgrep. If you need full functionalities, use `rg` command and eat its
output by batgrep.

- Some functionalities (e.g. preprocessor) are not supported
- Memory map is not used until `--mmap` flag is specified
- Context option is `-c`, not `-C`, and set to 10 by default

### Change color theme and layout

Default color theme is `Monokai Extended` respecting `bat` command's default. Other theme can be specified via `--theme` option.

```sh
grep -nH ... | batgrep --theme Nord
```

And batgrep respects `BAT_THEME` environment variable.

```sh
export BAT_THEME=OneHalfDark
```

Default layout is 'grid' respecting `bat` command's default. To print the matches without border lines, `--no-grid` option is
available.

```sh
grep -nH ... | batgrep --no-grid
```

And batgrep respencts `BAT_STYLE` environment variable. When `plain` or `header` or `numbers` is set, batgrep removes border
lines.

```sh
export BAT_STYLE=numbers
```

### Command options

- Common options
  - `--context` (`-c`): Lines of leading and trailing context surrounding each match
  - `--no-grid` (`-G`): Remove border lines for more compact output
  - `--tab`: Number of spaces for tab character
  - `--theme`: Theme for syntax highlighting
  - `--list-themes`: List all theme names available for --theme option
- Only for builtin ripgrep
  - `--no-ignore`: Don't respect ignore files (.gitignore, .ignore, etc.)
  - `--ignore-case` (`-i`): When this flag is provided, the given patterns will be searched case insensitively
  - `--smart-case` (`-S`): Search case insensitively if the pattern is all lowercase. Search case sensitively otherwise
  - `--glob` (`-g`): Include or exclude files and directories for searching that match the given glob
  - `--glob-case-insensitive`: Process glob patterns given with the -g/--glob flag case insensitively
  - `--fixed-strings` (`-F`): Treat the pattern as a literal string instead of a regular expression
  - `--word-regexp` (`-w`): Only show matches surrounded by word boundaries
  - `--follow` (`-L`): When this flag is enabled, batgrep will follow symbolic links while traversing directories
  - `--multiline` (`-U`): Enable matching across multiple lines
  - `--multiline-dotall`: Enable "dot all" in your regex pattern, which causes '.' to match newlines when multiline searching is enabled
  - `--crlf`: about(r"When enabled, batgrep will treat CRLF (`\r\n`) as a line terminator instead of just `\n`
  - `--mmap`: Search using memory maps when possible. mmap is disabled by default unlike batgrep
  - `--max-count` (`-m`): Limit the number of matching lines per file searched to NUM
  - `--max-depth`: Limit the depth of directory traversal to NUM levels beyond the paths given
  - `--max-filesize`: Ignore files larger than NUM in size
  - `--line-regexp` (`-x`): Only show matches surrounded by line boundaries. This is equivalent to putting ^...$ around all of the search patterns
  - `--pcre2` (`-P`): When this flag is present, batgrep will use the PCRE2 regex engine instead of its default regex engine

See `--help` for full list of options.

## Alternatives

Some other alternatives instead of using batgrep.

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

It works fine but batgrep is more optimized for this usage.

- When the matches are near enough, the lines are printed in one snippet.
- Performance is better than running `bat` process per matched line.
- batgrep is available where ShellScript is unavailable (e.g. PowerShell).

### Fuzzy finder like `fzf` with `bat` preview window

Fuzzy finder like [fzf][] privides a preview window functionality and `bat` can print the match previews.

```sh
grep -nH ... | \
    fzf --preview='bat --pager never --color always -H {2} -r {2}: -p {1}' --delimiter=:
```

This usage is great when you need the incremental search, but you need to check each preview of matches one by one.

batgrep focuses on surveying all the matches.

## License

batgrep is distributed under [the MIT license](./LICENSE.txt).

[batgrep]: https://github.com/rhysd/batgrep
[ripgrep]: https://github.com/BurntSushi/ripgrep
[bat]: https://github.com/sharkdp/bat
[cargo]: https://github.com/rust-lang/cargo
[ag]: https://github.com/ggreer/the_silver_searcher
[pt]: https://github.com/monochromegane/the_platinum_searcher
[fzf]: https://github.com/junegunn/fzf
