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

## Make a new release

Let's say we're releasing v1.2.3.

1. Make `v1.2.3` Git tag and push it to remote.
   ```sh
   git tag v1.2.3
   git push origin v1.2.3
   ```
2. [CI][release-ci] automatically creates a release page and uploads release binaries at [the releases page][releases].
3. Write up the release note at the release page.
4. Update changelog by [changelog-from-release][]
   ```sh
   changelog-from-release > CHANGELOG.md
   git add CHANGELOG.md
   git commit -m 'update chagnelog for v1.2.3 changes'
   ```
5. Update [the Homebrew formula][formula] by script
   ```sh
   ./HomebrewFormula/update.bash v1.2.3
   git add ./HomebrewFormula/hgrep.rb
   git commit -m 'update Homebrew formula to v1.2.3'
   ```
6. Push the changelog and formula updates
   ```sh
   git push
   ```

[new-issue]: https://github.com/rhysd/hgrep/issues/new
[new-pr]: https://github.com/rhysd/hgrep/pulls
[clippy]: https://github.com/rust-lang/rust-clippy
[rustfmt]: https://github.com/rust-lang/rustfmt
[repo]: https://github.com/rhysd/hgrep
[release-ci]: ./.github/workflows/release.yml
[releases]: https://github.com/rhysd/hgrep/releases
[changelog-from-release]: https://github.com/rhysd/changelog-from-release
[formula]: ./HomebrewFormula/hgrep.rb
