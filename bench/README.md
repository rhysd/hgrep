Benchmarks
==========

## Prerequisites

[npm][] is necessary since we use:

- `package-lock.json` as example of large file
- `node_modules` black hole as example of large directory

Create them by running `npm install`.

## Run benchmarks

```sh
# Run all benchmark suites
cargo bench --benches

# Run a specific benchmark suite
cargo bench --bench chunk

# Filter by benchmark name
cargo bench min_3_max_6

# Run specific 
```

## Compare benchmarking results

Use `--save-baseline` and `--baseline` to compare two results.

```sh
git checkout master
cargo bench -- --save-baseline base

git checkout feature
cargo bench -- --baseline change
```

Using [critcmp][] may be more useful.

## Notes

- To suppress massive outputs to stdout, `printer` benchmark suite uses [`gag` crate][gag], which is not available on Windows.

[critcmp]: https://github.com/BurntSushi/critcmp
[npm]: https://www.npmjs.com/
[gag]: https://docs.rs/gag/
