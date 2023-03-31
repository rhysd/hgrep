<a name="v0.3.2"></a>
# [v0.3.2](https://github.com/rhysd/hgrep/releases/tag/v0.3.2) - 31 Mar 2023

- Update `bat` from 0.22 to [0.23](https://github.com/sharkdp/bat/releases/tag/v0.23.0). This improves performance on macOS when using `-p bat`.
- Add support for Ada syntax highlighting
- Update dependencies
  - Remove `remove_dir_all` crate to avoid CVE-2022-21658
  - Replace unmaintained `ansi_term` crate with `nu-ansi-term` crate
  - Update clap crates to generate better completion scripts and man page

[Changes][v0.3.2]


<a name="v0.3.1"></a>
# [v0.3.1](https://github.com/rhysd/hgrep/releases/tag/v0.3.1) - 31 Jan 2023

- Fix parsing some command line options
  - Fix `--glob` was not repeatable
  - Fix `--ignore-case` and `--smart-case` should override each other
  - Fix `--line-regexp` and `--word-regexp` should override each other

[Changes][v0.3.1]


<a name="v0.3.0"></a>
# [v0.3.0](https://github.com/rhysd/hgrep/releases/tag/v0.3.0) - 21 Jan 2023

- Update `syntect` dependency to v5.0.0. Thanks to lazy loading, this change makes loading assets at startup about **70%** faster. The small benchmark showed `hgrep` command was [1.7x faster](https://github.com/rhysd/hgrep/commit/c26db0fb924dcea466c00dde84b90d64bce7f461) when searching a small file with `-p syntect` compared to v0.2.8.
- Update `bat` dependency from 0.20 to 0.22. This introduces several improvements and fixes which were recently added to bat when using `-p bat`.
- Add `--generate-man-page` flag to generate a manual page file. Save the output to your `man` directory to show the help with `man` command. If you install hgrep with Homebrew, it will be automatically generated.
  ```sh
  hgrep --generate-man-page > /usr/local/share/man/man1/hgrep.1
  man hgrep
  ```
- Add several syntax highlighting for configuration files (Git configs, Fish history, SSH config, Nginx config, ...).
- Wrap the `--help` output looking at the terminal width. The output is more compact than v0.2.8.
- Improve error handling when failing to enable ANSI color sequence support on Windows.

[Changes][v0.3.0]


<a name="v0.2.8"></a>
# [v0.2.8](https://github.com/rhysd/hgrep/releases/tag/v0.2.8) - 10 Jan 2023

- Fix some command line boolean flags wrongly took an argument. (thanks [@Ryooooooga](https://github.com/Ryooooooga), [#15](https://github.com/rhysd/hgrep/issues/15))

[Changes][v0.2.8]


<a name="v0.2.7"></a>
# [v0.2.7](https://github.com/rhysd/hgrep/releases/tag/v0.2.7) - 04 Jan 2023

- Fix crash when reading from `rg --vimgrep`. Note that `--vimgrep` flag is not assumed by hgrep. Please use `rg -nH`. ([#13](https://github.com/rhysd/hgrep/issues/13))
- Fix errors are not reported when they are caused by the second match or later.
- Update dependencies to the latest. Especially migrating to clap v4 improved the `--help` output.
- Migrate to Rust 2021 edition.

[Changes][v0.2.7]


<a name="v0.2.6"></a>
# [v0.2.6](https://github.com/rhysd/hgrep/releases/tag/v0.2.6) - 27 May 2022

- `x86_64-unknown-linux-musl` release binary now links libc statically ([#10](https://github.com/rhysd/hgrep/issues/10))
- Replace `rgb2ansi256` crate with `ansi_colors` crate

[Changes][v0.2.6]


<a name="v0.2.5"></a>
# [v0.2.5](https://github.com/rhysd/hgrep/releases/tag/v0.2.5) - 23 Apr 2022

- Add pre-built binary for AArch64 Linux. ([#9](https://github.com/rhysd/hgrep/issues/9))

[Changes][v0.2.5]


<a name="v0.2.4"></a>
# [v0.2.4](https://github.com/rhysd/hgrep/releases/tag/v0.2.4) - 17 Apr 2022

- Update `bat` crate dependency to v0.20.0.
- Highlight clang-format configuration file.

[Changes][v0.2.4]


<a name="v0.2.3"></a>
# [v0.2.3](https://github.com/rhysd/hgrep/releases/tag/v0.2.3) - 02 Feb 2022

- Update dependencies including `bat` v0.19 and `clap` v3
- Build binaries with the latest Rust compiler v1.58.1

[Changes][v0.2.3]


<a name="v0.2.2"></a>
# [v0.2.2](https://github.com/rhysd/hgrep/releases/tag/v0.2.2) - 11 Dec 2021

- Fix a build failure since new RC version of `clap` crate was released.
- Fix a dynamic link error of pcre2 library by linking the library statically. The error could happen when you installed Homebrew to non-default location on macOS ([#6](https://github.com/rhysd/hgrep/issues/6)).
- Add `--regex-size-limit` option for built-in grep feature.
- Add `--dfa-size-limit` option for built-in grep feature.
- Use Rust compiler v1.57 to build binaries.

[Changes][v0.2.2]


<a name="v0.2.1"></a>
# [v0.2.1](https://github.com/rhysd/hgrep/releases/tag/v0.2.1) - 13 Nov 2021

- Heuristic algorithm to choose the foreground color of matched regions was improved. Now hgrep generates multiple candidates for the foreground color, and chooses one of them looking at the color distances from the background color.
  - Example with `Coldark-Dark` theme. Please find the 'let' matched regions in the following screenshots. The foreground color is easier to see in v0.2.1 than v0.2.0.
    | v0.2.0 | v0.2.1 |
    |--------|--------|
    | <img width="405" alt="example in v0.2.0" src="https://user-images.githubusercontent.com/823277/141615175-110473d4-1821-43c0-aea4-79c7cafea533.png"> | <img width="407" alt="example in v0.2.1" src="https://user-images.githubusercontent.com/823277/141615189-a46579ba-edfa-4d3a-ab5b-e2e8c817d00b.png"> |
- Add new `Material` theme. It is a very popular low-contrast color theme. Try it by `hgrep --theme Material`.
  <img width="584" alt="Material" src="https://user-images.githubusercontent.com/823277/141615211-4dd68326-aafe-4851-afa3-6eeacee56d71.png">
- Add new `Carbonight` theme. It is a minimal monotone color theme. Some people feel that too colorful outputs are hard to see. This color theme might fit to such people.
  <img width="584" alt="Carbonight" src="https://user-images.githubusercontent.com/823277/141615244-346a46ca-da87-4981-ab7a-75b74d750d23.png">
- Built-in grep allows K/M/G suffix at `--max-filesize` option to specify a file size easily.
  ```sh
  # Search files whose size is smaller than 10 MiB
  hgrep --maxfilesize 10M ...
  ```
- Built-in grep adds new flag `--invert-match` for invert matching. It shows lines that do not match the given pattern.
- Built-in grep adds new flag `--one-file-system`. When enabled, the search will not cross file system boundaries relative to where it started from.
- Built-in grep adds new short flag`-.` as alias of long flag `--hidden`.
- Built-in grep adds new flag `--no-unicode` which disables Unicode-aware search.
- Built-in grep improves the output from `--type-list`. Now types are printed in bold texts which is easier to see.
- Syntax assets were updated to the latest. They improve some syntax highlight detection (for example, `vimrc` for Vim files) and solve some highlighting issues.
- Fix a broken pipe error when `hgrep` command is piped to a pager command like `less`. This happened when `less` exits earlier than `hgrep` command, for example, when you immediately quit a pager by `q` without scrolling the output to the end. In the case, `hgrep` still tried to output the result to stdout even if the pipe had already been closed and it caused a broken pipe error. In v0.2.1, `hgrep` correctly ignores such broken pipe errors.
- Fix `--no-wrap` deprecated flag was not removed at v0.2.0. Use `--wrap` instead if you used the flag.
- Fix checksum of downloaded package via Homebrew on arm64 macOS.
- Fix `--type-list` flag did not print types when a pattern argument is not given.
- (Dev) Move `asset-builder` tool directory to `assets/builder`.
- (Dev) The script to update test snapshots is now 25x faster.
- (Dev) CI job to run clippy and rustfmt is now 6x faster.

[Changes][v0.2.1]


<a name="v0.2.0"></a>
# [v0.2.0](https://github.com/rhysd/hgrep/releases/tag/v0.2.0) - 06 Nov 2021

- **BREAKING** The default printer is now `syntect`. It has the following benefits. I tested it for several weeks and it seems stable. See [the section in README](https://github.com/rhysd/hgrep#bat-printer-vs-syntect-printer) to know the difference between `bat` printer and `syntect` printer.
  - Performance is 2x to 4x faster
  - Output layout and highlighting are optimized; line number highlight and matched regions at matched line
  - Support background color with `--background`
  - Color themes are optimized
- **BREAKING** The default value of `--min-context` was changed from 5 to 3. And the default value of `--max-context` was changed from 10 to 6. This is because it turned out that the previous default values were too large for surveying the search results.
- **BREAKING** Since themes for `syntect` printer are now managed by ourselves (see below), `syntect` printer no longer looks at `BAT_THEME` and `BAT_STYLE` environment variables. To set the default theme and layout, use shell's command alias. See [the document](https://github.com/rhysd/hgrep#change-color-theme-and-layout) for more details.
- `syntect` printer now renders more accurate colors by considering alpha values of colors by blending them with background colors. In v0.1.9, alpha values were simply ignored. For example, gutter color with `Nord` theme was wrongly very light at v0.1.9.
  - Before (v0.1.9):
    <img width="584" alt="v0.1.9 Nord" src="https://user-images.githubusercontent.com/823277/140617940-a16aad7e-8b8b-46f2-aba8-158d62559676.png">
  - After (v0.2.0):
    <img width="584" alt="v0.2.0 Nord" src="https://user-images.githubusercontent.com/823277/140617970-fa1bef89-42bc-464a-9c5c-52e3944d2d15.png">
- Manage our own theme set to optimize themes for our use cases. Comparing with bat's theme assets, some themes are removed whose line highlight color and/or searched text color are obscure or hard to see. And some new famous themes are added. The theme assets are managed in [`assets` directory](https://github.com/rhysd/hgrep/tree/main/assets).
  - [ayu](https://github.com/dempfi/ayu): Famous vivid color theme
    | `ayu-dark` | `ayu-mirage` | `ayu-light` |
    |------------|--------------|-------------|
    | <img width="577" alt="ayu-dark" src="https://user-images.githubusercontent.com/823277/140617846-ad16d72a-3467-484d-9700-9df8055a2288.png"> | <img width="577" alt="ayu-mirage" src="https://user-images.githubusercontent.com/823277/140617854-3a76487a-5912-491d-85e3-0091596002c0.png"> | <img width="577" alt="ayu-light" src="https://user-images.githubusercontent.com/823277/140617864-cc3a62c3-081a-453a-b8d2-2fb6061a3061.png"> |
  - [predawn](https://github.com/jamiewilson/predawn): Famous low-contrast color theme
    <img width="577" alt="predawn" src="https://user-images.githubusercontent.com/823277/140617876-f18dc76c-9694-4d00-84d0-7af671554517.png">
  - [cyanide](https://github.com/lefoy/cyanide-theme): Famous minimal color theme
    <img width="584" alt="cyanide" src="https://user-images.githubusercontent.com/823277/140618295-496ba46f-8500-44e5-85b2-d4094a049b68.png">
- Output of `--list-themes` is much improved. It shows sample outputs per theme so that users can know what they look like. Options related to outputs like `--background` and `--no-grid` are reflected to the sample outputs. At v0.1.9, only theme names were printed so users needed to try the themes by themselves.
  <img width="584" alt="list themes output example" src="https://user-images.githubusercontent.com/823277/140618330-37d418be-c7ea-4b98-b57e-a7fabefe5199.png">
- Linux x86_64 musl target was added to pre-built releases. Find `hgrep-*-x86_64-unknown-linux-musl.zip` in released assets. Note that this binary is not tested. ([#5](https://github.com/rhysd/hgrep/issues/5))
- Depend on `ansi_term` crate only when targeting Windows. It reduces number of dependencies when `bat-printer` is not enabled.
- Improve a compile error when both feature `syntect-printer` and `bat-printer` are disabled.
- Describe the exit status of `hgrep` command and versioning of this project in [the readme document](https://github.com/rhysd/hgrep#readme).
- Fix rendering `ansi` theme was broken. The theme is for old terminals which only supports 16 colors.
- Fix `--first-only` did not work with `bat` printer.
- Fix the background color in file header when `--background` is specified


[Changes][v0.2.0]


<a name="v0.1.9"></a>
# [v0.1.9](https://github.com/rhysd/hgrep/releases/tag/v0.1.9) - 01 Nov 2021

- Support multiple regions highlighting. In v0.1.8, matched region highlighting was added but it only highlighted the first match in the line. Now all matched regions are highlighted. Note that region highlighting is available when using hgrep in combination of `syntect-printer` and `ripgrep` features
  - v0.1.8:
    <img width="234" alt="multi regions before screenshot" src="https://user-images.githubusercontent.com/823277/139637214-8ec7e6cf-33a5-4df2-b334-794c6641b13e.png">
  - v0.1.9:
    <img width="221" alt="multi regions after screenshot" src="https://user-images.githubusercontent.com/823277/139637302-03fd69b7-865c-4636-8b3f-b19eb697c3e9.png">
- Add `--ascii-lines` flag for terminals which does not support rendering unicode characters well. With this flag, unicode characters like '│' or '─' are replaced with ASCII characters '|' or '-'. This feature is only supported by `syntect-printer` (use `-p syntect`).
  <img width="682" alt="ascii lines screenshot" src="https://user-images.githubusercontent.com/823277/139636882-b23caa7e-d92d-4c49-a5af-9021dce6d92a.png">
- Add `--first-only` (`-f`) flag to show only the first snippet per file. This is useful when you want to look around the results.
- Fallback to a minimap border color when no gutter background color is found.
- Reduce number of redundant color codes output to stdout by **about 21.5%** in test cases. This also improves performance by **about 6%**. See [the commit](https://github.com/rhysd/hgrep/commit/3f95d9a854bdb875194ed088887635ebc77a9269) for details.
- Performance of built-in grep was improved **20~80%** when there are so many files to search. Previously the implementation collected all paths before searching a pattern in them, but with this improvement, the paths are now streamed. See [the commit](https://github.com/rhysd/hgrep/commit/693ea1810c637a4939c0f76ae2457f7bcd691179) for details.
- Use [mimalloc](https://github.com/microsoft/mimalloc) as global allocator for better performance. This improves performance by **0~55%** in our benchmarks. See [the commit](https://github.com/rhysd/hgrep/commit/2587e82683d66722c18c237f3fcdfe33cabb9c8b) for details.
- (Dev) Running unit tests is about 8.5x faster by caching assets for syntax highlighting.

[Changes][v0.1.9]


<a name="v0.1.8"></a>
# [v0.1.8](https://github.com/rhysd/hgrep/releases/tag/v0.1.8) - 27 Oct 2021

- `syntect-printer` supports text-wrapping. Longer lines than terminal width are now wrapped by default. It can handle wide characters including special emojis with zero-width joiner (U+200D) like 👨‍👩‍👧‍👦
  <img width="521" alt="screenshot" src="https://user-images.githubusercontent.com/823277/139065592-8d18f8a0-9b10-49c7-8901-fd892d100792.png">
- `syntect-printer` highlights matched regions in matched lines with a searched text color. Since match positions in matched lines are not included in output from `grep -nH`, currently this is only supported by combination of `syntect-printer` feature and `ripgrep` feature
- `syntect-printer` now uses light dashed lines for the separator of snippets: `╶╶╶╶╶╶╶╶╶╶╶╶`
- Add `--wrap MODE` option where `MODE` is one of `char` or `never` (the default value is `char`). More modes may be implemented in the future
- In favor of `--wrap` option, `--no-wrap` flag is now deprecated and will be removed at v0.2.0. Use `--wrap never` instead
- When building binaries for Windows, link C runtime statically. This avoid depending on vcruntime DLL at runtime
- Critical section of `syntect-printer` was optimized. It slightly improved performance (around 4% faster in benchmarks)
- Enable thin LTO for release build. It slightly improved performance (0~6% faster in benchmarks). See [the commit](https://github.com/rhysd/hgrep/commit/226c4b565550f1da550024ca898819f2431e052f) for details

[Changes][v0.1.8]


<a name="v0.1.7"></a>
# [v0.1.7](https://github.com/rhysd/hgrep/releases/tag/v0.1.7) - 24 Oct 2021

- Fix highlighting was broken on 256 colors terminals when using `bat-printer`.
- `bat-printer` enables text wrapping by default as `bat` command does. `--no-wrap` can disable text wrapping.
- `bat-printer` now looks at bat's cache directory when `--custom-assets` flag is given. This is useful if you use some custom syntax highlighting or theme. Note that this may not work fine with some versions of `bat` command.
- `bat-printer` automatically uses 'ansi' theme for terminals which enable only 16 colors since other themes don't work.
- Add `--terminal-width` option to give the width of terminal explicitly. This is useful when piping the results to other command like `less`.
- Fix build failure due to lack of assets ([#4](https://github.com/rhysd/hgrep/issues/4)).
- Fix some newlines were missing when printing results with `syntect-printer`.
- Use `terminal_size` crate directly instead of using `console` crate. It removes 3 dependencies when `bat-printer` feature is not enabled.
- The document has been improved. Especially if you like a pager such as `less`, I recommend to check ['Set default command options'](https://github.com/rhysd/hgrep#set-default-command-options) section.
- (Dev) Several tests and benchmarks for `syntect-printer` were added.

[Changes][v0.1.7]


<a name="v0.1.6"></a>
# [v0.1.6](https://github.com/rhysd/hgrep/releases/tag/v0.1.6) - 23 Oct 2021

- Add new experimental `syntect-printer` feature built with [syntect](https://github.com/trishume/syntect) library.
  - It is much faster than current printer built on bat (2x~4x faster).
  - Its output layout is optimized for our use case. For example, line numbers at matches are highlighted in different color.
  - It supports painting background colors with `--background` flag. This is useful when your favorite theme does not fit to your terminal's background color.
  - See [`bat` printer v.s. `syntect` printer](https://github.com/rhysd/hgrep#bat-printer-vs-syntect-printer) section for comparison of the two printers.
- Add `--printer` (`-p`) flag to specify printer to use. It takes argument `bat` or `syntect`. `-p syntect` enables the new experimental printer
- `bat` printer is now optional through `bat-printer` feature gate. Note that at least `bat-printer` or `syntect-printer` must be enabled. Both printers are enabled by default. See [Feature flags](https://github.com/rhysd/hgrep#feature-flags) section for more details.
- hgrep is now available for NetBSD. See [the instruction](https://github.com/rhysd/hgrep#for-netbsd) (thanks [@0323pin](https://github.com/0323pin), [#3](https://github.com/rhysd/hgrep/issues/3))

[Changes][v0.1.6]


<a name="v0.1.5"></a>
# [v0.1.5](https://github.com/rhysd/hgrep/releases/tag/v0.1.5) - 20 Oct 2021

- Always use a relative path in header of output
- Fix an output is broken due to ANSI color sequence on Windows

[Changes][v0.1.5]


<a name="v0.1.4"></a>
# [v0.1.4](https://github.com/rhysd/hgrep/releases/tag/v0.1.4) - 19 Oct 2021

- Fix compile error on `cargo install` due to new release of `clap` crate v3.0.0-beta.5 ([#2](https://github.com/rhysd/hgrep/issues/2))
- Add how to install `hgrep` command with [MacPorts](https://www.macports.org/). See [the document](https://github.com/rhysd/hgrep#via-macports) for more details (thanks [@herbygillot](https://github.com/herbygillot), [#1](https://github.com/rhysd/hgrep/issues/1))

[Changes][v0.1.4]


<a name="v0.1.3"></a>
# [v0.1.3](https://github.com/rhysd/hgrep/releases/tag/v0.1.3) - 19 Oct 2021

- Heuristics on calculating context lines is 1.3x faster by using optimized [memchr](https://docs.rs/memchr/2.4.1/memchr/) implementation when the searched file is large
- [Homebrew](http://brew.sh/) is now supported for managing `hgrep` command on macOS or Linux. See [the installation instruction](https://github.com/rhysd/hgrep#via-homebrew) for more details
- Add `--grid` flag as an opposite of `--no-grid` flag
- Add [CONTRIBUTING.md](https://github.com/rhysd/hgrep/blob/main/CONTRIBUTING.md) which describes the development of this project
- (Dev) Add [some benchmark suites](https://github.com/rhysd/hgrep/tree/main/hgrep-bench) for each parts of this program to track performance

[Changes][v0.1.3]


<a name="v0.1.2"></a>
# [v0.1.2](https://github.com/rhysd/hgrep/releases/tag/v0.1.2) - 17 Oct 2021

- Fix printing tab characters. Now default tab width is 4 (can be configured with `--tab` option).
- Fix exit status is always 0 when no error happens. Grep tool should return non-zero exit status when no match was found.
- Add feature to generate shell completion scripts for Bash, Zsh, Fish, PowerShell, and Elvish. Check `--generate-completion-script` option.
- Printing results is now much faster. It is [3.3x faster than previous](https://github.com/rhysd/hgrep/commit/8655b801b40f8b3f7d4d343cae185604fa918d5b).

[Changes][v0.1.2]


<a name="v0.1.1"></a>
# [v0.1.1](https://github.com/rhysd/hgrep/releases/tag/v0.1.1) - 16 Oct 2021

First release :tada:

See [the readme document](https://github.com/rhysd/hgrep#readme) for the usage.

[Changes][v0.1.1]


[v0.3.2]: https://github.com/rhysd/hgrep/compare/v0.3.1...v0.3.2
[v0.3.1]: https://github.com/rhysd/hgrep/compare/v0.3.0...v0.3.1
[v0.3.0]: https://github.com/rhysd/hgrep/compare/v0.2.8...v0.3.0
[v0.2.8]: https://github.com/rhysd/hgrep/compare/v0.2.7...v0.2.8
[v0.2.7]: https://github.com/rhysd/hgrep/compare/v0.2.6...v0.2.7
[v0.2.6]: https://github.com/rhysd/hgrep/compare/v0.2.5...v0.2.6
[v0.2.5]: https://github.com/rhysd/hgrep/compare/v0.2.4...v0.2.5
[v0.2.4]: https://github.com/rhysd/hgrep/compare/v0.2.3...v0.2.4
[v0.2.3]: https://github.com/rhysd/hgrep/compare/v0.2.2...v0.2.3
[v0.2.2]: https://github.com/rhysd/hgrep/compare/v0.2.1...v0.2.2
[v0.2.1]: https://github.com/rhysd/hgrep/compare/v0.2.0...v0.2.1
[v0.2.0]: https://github.com/rhysd/hgrep/compare/v0.1.9...v0.2.0
[v0.1.9]: https://github.com/rhysd/hgrep/compare/v0.1.8...v0.1.9
[v0.1.8]: https://github.com/rhysd/hgrep/compare/v0.1.7...v0.1.8
[v0.1.7]: https://github.com/rhysd/hgrep/compare/v0.1.6...v0.1.7
[v0.1.6]: https://github.com/rhysd/hgrep/compare/v0.1.5...v0.1.6
[v0.1.5]: https://github.com/rhysd/hgrep/compare/v0.1.4...v0.1.5
[v0.1.4]: https://github.com/rhysd/hgrep/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/rhysd/hgrep/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/rhysd/hgrep/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/rhysd/hgrep/tree/v0.1.1

<!-- Generated by https://github.com/rhysd/changelog-from-release v3.7.0 -->
