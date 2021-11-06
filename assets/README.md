## Assets imported from [bat](https://github.com/sharkdp/bat)

`syntaxes.bin` and `ansi.tmTheme` were imported from bat `ed3246c423932561435d45c50fd8cd9e06add7f5`. They're licensed with
[the MIT license](./bat-LICENSE-MIT).

## How to update `syntaxes.bin`

Copy from [bat's assets directory](https://github.com/sharkdp/bat/tree/master/assets).

## How to update `themes.bin`

Run [assets-builder](../assets-builder).

## How to add new color theme

1. Add the color theme repository to [`./submodules`](./submodules) as Git submodule.
2. Edit [`assets-builder/src/main.rs`](../assets-builder/src/main.rs). Add the path to `.tmTheme` file to `THEME_PATHS` constant.
3. Run assets-builder to re-generate `themes.bin`.

**Note:** [The CI workflow](.github/workflows/assets.yaml) checks if the `themes.bin` file is up-to-date.
