#!/bin/bash

set -e -o pipefail

if [ ! -d .git ]; then
    echo 'This script must be run at root directory of this repository' >&2
    exit 1
fi

if [[ "$COLORTERM" != "truecolor" ]]; then
    echo 'This script must be run in a terminal which supports true colors (24-bit colors)' >&2
    exit 1
fi

set -x

cargo run -- 'match to this line' -p syntect --term-width 80 --theme ansi ./testdata/syntect/ansi16_colors.rs  > ./testdata/syntect/ansi16_colors.out
COLORTERM='' cargo run -- 'match to this line' -p syntect --term-width 80 ./testdata/syntect/ansi256_colors.rs > ./testdata/syntect/ansi256_colors.out
cargo run -- 'match to this line' -p syntect --term-width 80 --background ./testdata/syntect/background.rs     > ./testdata/syntect/background.out
cargo run -- 'match to this line' -p syntect --term-width 80              ./testdata/syntect/default.rs        > ./testdata/syntect/default.out
cargo run -- 'match to this line' -p syntect --term-width 80 --tab 0      ./testdata/syntect/hard_tab.rs       > ./testdata/syntect/hard_tab.out
cargo run -- 'match to this line' -p syntect --term-width 80              ./testdata/syntect/long_line.rs      > ./testdata/syntect/long_line.out
cargo run -- 'match to this line' -p syntect --term-width 80 --background ./testdata/syntect/long_line_bg.rs   > ./testdata/syntect/long_line_bg.out
cargo run -- 'match to this line' -p syntect --term-width 80 --no-grid    ./testdata/syntect/no_grid.rs        > ./testdata/syntect/no_grid.out
cargo run -- 'match to this line' -p syntect --term-width 80 --tab 2      ./testdata/syntect/tab_width_2.rs    > ./testdata/syntect/tab_width_2.out
cargo run -- 'match to this line' -p syntect --term-width 80 --theme Nord ./testdata/syntect/theme.rs          > ./testdata/syntect/theme.out

cat ./testdata/syntect/ansi16_colors.out
cat ./testdata/syntect/ansi256_colors.out
cat ./testdata/syntect/background.out
cat ./testdata/syntect/default.out
cat ./testdata/syntect/hard_tab.out
cat ./testdata/syntect/long_line.out
cat ./testdata/syntect/no_grid.out
cat ./testdata/syntect/tab_width_2.out
cat ./testdata/syntect/theme.out
