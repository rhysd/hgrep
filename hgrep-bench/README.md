Benchmarks
==========

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

Use [critcmp][].

```sh
git checkout master
cargo bench -- --save-baseline base

git checkout feature
cargo bench -- --save-baseline change

critcmp base change
```

[critcmp]: https://github.com/BurntSushi/critcmp
