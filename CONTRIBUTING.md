Contributing to [hgrep][repo]
=============================

## Bug reporting

Please [make an issue][new-issue] on GitHub.  Ensure to describe how to reproduce the bug.

## Checks before making a pull request

Pull request is always welcome. Please [make a pull request][new-pr] on GitHub.

Ensure all tests pass with enabling/disabling builtin ripgrep.

```sh
# Run all unit tests
cargo test
# Run all unit tests disabling builtin ripgrep
cargo test --no-default-features
```

And do sanity check with enabling/disabling builtin ripgrep.

```sh
# Check results in your terminal
cargo run -- Printer ./src
# Check results disabling builtin grep
grep -nH Printer -R ./src | cargo run --no-default-features
```

Optionally check [clippy][] linter and [rustfmt][] code formatter.

```sh
cargo clippy
cargo fmt
```

## UI tests for `syntect-printer`

Some tests in `src/syntect.rs` check UI in output from the printer. Expected data are put in `testdata/syntect/` directory.
For example, the result of printing `testdata/syntect/default.rs` with some options is `testdata/syntect/default.out`.

These expected data can be updated with script at `testdata/update_syntect_uitest.bash`. When some UI logic is updated and
the expected data printed to stdout changes, run the script to update `testdata/syntect/*.out`.

The script prints the expected outputs. Review they are correct manually.

## Make a new release

Let's say we're releasing v1.2.3.

1. Modify `version` in [Cargo.toml](./Cargo.toml), run `cargo build`, and commit changes.
2. Make `v1.2.3` Git tag and push it to remote.
   ```sh
   git tag v1.2.3
   git push origin v1.2.3
   ```
3. [CI][release-ci] automatically creates a release page, uploads release binaries at [the releases page][releases], and updates
   [the Homebrew formula][formula] with `HomebrewFormula/update.bash`.
4. Write up the release note at the release page.
5. Update changelog by [changelog-from-release][]
   ```sh
   changelog-from-release > CHANGELOG.md
   git add CHANGELOG.md
   git commit -m 'update chagnelog for v1.2.3 changes'
   git push
   ```
6. Make new release at crates.io by `cargo publish`.

## Benchmarking

Benchmarks are put in [a separate crate](./bench) to avoid adding criterion as test dependencies. To run benchmarks,
see [the benchmark README file](./bench/README.md).

[new-issue]: https://github.com/rhysd/hgrep/issues/new
[new-pr]: https://github.com/rhysd/hgrep/pulls
[clippy]: https://github.com/rust-lang/rust-clippy
[rustfmt]: https://github.com/rust-lang/rustfmt
[repo]: https://github.com/rhysd/hgrep
[release-ci]: ./.github/workflows/release.yml
[releases]: https://github.com/rhysd/hgrep/releases
[changelog-from-release]: https://github.com/rhysd/changelog-from-release
[formula]: ./HomebrewFormula/hgrep.rb
