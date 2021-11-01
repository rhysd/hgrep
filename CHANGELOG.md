<a name="v0.1.9"></a>
# [v0.1.9](https://github.com/rhysd/batgrep/releases/tag/v0.1.9) - 01 Nov 2021

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
# [v0.1.8](https://github.com/rhysd/batgrep/releases/tag/v0.1.8) - 27 Oct 2021

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
# [v0.1.7](https://github.com/rhysd/batgrep/releases/tag/v0.1.7) - 24 Oct 2021

- Fix highlighting was broken on 256 colors terminals when using `bat-printer`.
- `bat-printer` enables text wrapping by default as `bat` command does. `--no-wrap` can disable text wrapping.
- `bat-printer` now looks at bat's cache directory when `--custom-assets` flag is given. This is useful if you use some custom syntax highlighting or theme. Note that this may not work fine with some versions of `bat` command.
- `bat-printer` automatically uses 'ansi' theme for terminals which enable only 16 colors since other themes don't work.
- Add `--terminal-width` option to give the width of terminal explicitly. This is useful when piping the results to other command like `less`.
- Fix build failure due to lack of assets (#4).
- Fix some newlines were missing when printing results with `syntect-printer`.
- Use `terminal_size` crate directly instead of using `console` crate. It removes 3 dependencies when `bat-printer` feature is not enabled.
- The document has been improved. Especially if you like a pager such as `less`, I recommend to check ['Set default command options'](https://github.com/rhysd/hgrep#set-default-command-options) section.
- (Dev) Several tests and benchmarks for `syntect-printer` were added.

[Changes][v0.1.7]


<a name="v0.1.6"></a>
# [v0.1.6](https://github.com/rhysd/batgrep/releases/tag/v0.1.6) - 23 Oct 2021

- Add new experimental `syntect-printer` feature built with [syntect](https://github.com/trishume/syntect) library.
  - It is much faster than current printer built on bat (2x~4x faster).
  - Its output layout is optimized for our use case. For example, line numbers at matches are highlighted in different color.
  - It supports painting background colors with `--background` flag. This is useful when your favorite theme does not fit to your terminal's background color.
  - See [`bat` printer v.s. `syntect` printer](https://github.com/rhysd/hgrep#bat-printer-vs-syntect-printer) section for comparison of the two printers.
- Add `--printer` (`-p`) flag to specify printer to use. It takes argument `bat` or `syntect`. `-p syntect` enables the new experimental printer
- `bat` printer is now optional through `bat-printer` feature gate. Note that at least `bat-printer` or `syntect-printer` must be enabled. Both printers are enabled by default. See [Feature flags](https://github.com/rhysd/hgrep#feature-flags) section for more details.
- hgrep is now available for NetBSD. See [the instruction](https://github.com/rhysd/hgrep#for-netbsd) (thanks @0323pin, #3)

[Changes][v0.1.6]


<a name="v0.1.5"></a>
# [v0.1.5](https://github.com/rhysd/batgrep/releases/tag/v0.1.5) - 20 Oct 2021

- Always use a relative path in header of output
- Fix an output is broken due to ANSI color sequence on Windows

[Changes][v0.1.5]


<a name="v0.1.4"></a>
# [v0.1.4](https://github.com/rhysd/batgrep/releases/tag/v0.1.4) - 19 Oct 2021

- Fix compile error on `cargo install` due to new release of `clap` crate v3.0.0-beta.5 (#2)
- Add how to install `hgrep` command with [MacPorts](https://www.macports.org/). See [the document](https://github.com/rhysd/hgrep#via-macports) for more details (thanks @herbygillot, #1)

[Changes][v0.1.4]


<a name="v0.1.3"></a>
# [v0.1.3](https://github.com/rhysd/batgrep/releases/tag/v0.1.3) - 19 Oct 2021

- Heuristics on calculating context lines is 1.3x faster by using optimized [memchr](https://docs.rs/memchr/2.4.1/memchr/) implementation when the searched file is large
- [Homebrew](http://brew.sh/) is now supported for managing `hgrep` command on macOS or Linux. See [the installation instruction](https://github.com/rhysd/hgrep#via-homebrew) for more details
- Add `--grid` flag as an opposite of `--no-grid` flag
- Add [CONTRIBUTING.md](https://github.com/rhysd/hgrep/blob/main/CONTRIBUTING.md) which describes the development of this project
- (Dev) Add [some benchmark suites](https://github.com/rhysd/hgrep/tree/main/hgrep-bench) for each parts of this program to track performance

[Changes][v0.1.3]


<a name="v0.1.2"></a>
# [v0.1.2](https://github.com/rhysd/batgrep/releases/tag/v0.1.2) - 17 Oct 2021

- Fix printing tab characters. Now default tab width is 4 (can be configured with `--tab` option).
- Fix exit status is always 0 when no error happens. Grep tool should return non-zero exit status when no match was found.
- Add feature to generate shell completion scripts for Bash, Zsh, Fish, PowerShell, and Elvish. Check `--generate-completion-script` option.
- Printing results is now much faster. It is [3.3x faster than previous](https://github.com/rhysd/hgrep/commit/8655b801b40f8b3f7d4d343cae185604fa918d5b).

[Changes][v0.1.2]


<a name="v0.1.1"></a>
# [v0.1.1](https://github.com/rhysd/batgrep/releases/tag/v0.1.1) - 16 Oct 2021

First release :tada:

See [the readme document](https://github.com/rhysd/hgrep#readme) for the usage.

[Changes][v0.1.1]


[v0.1.9]: https://github.com/rhysd/batgrep/compare/v0.1.8...v0.1.9
[v0.1.8]: https://github.com/rhysd/batgrep/compare/v0.1.7...v0.1.8
[v0.1.7]: https://github.com/rhysd/batgrep/compare/v0.1.6...v0.1.7
[v0.1.6]: https://github.com/rhysd/batgrep/compare/v0.1.5...v0.1.6
[v0.1.5]: https://github.com/rhysd/batgrep/compare/v0.1.4...v0.1.5
[v0.1.4]: https://github.com/rhysd/batgrep/compare/v0.1.3...v0.1.4
[v0.1.3]: https://github.com/rhysd/batgrep/compare/v0.1.2...v0.1.3
[v0.1.2]: https://github.com/rhysd/batgrep/compare/v0.1.1...v0.1.2
[v0.1.1]: https://github.com/rhysd/batgrep/tree/v0.1.1

 <!-- Generated by https://github.com/rhysd/changelog-from-release -->
