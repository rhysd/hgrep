#!/bin/bash

set -e -o pipefail

if [ ! -d .git ]; then
    echo 'This script must be run at root directory of this repository' >&2
    exit 1
fi

set -x

export COLORTERM=truecolor

cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --theme ansi              ./testdata/syntect/ansi16_colors.rs                > ./testdata/syntect/ansi16_colors.out
COLORTERM='' cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80              ./testdata/syntect/ansi256_colors.rs               > ./testdata/syntect/ansi256_colors.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/background.rs                   > ./testdata/syntect/background.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/default.rs                      > ./testdata/syntect/default.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --tab 0                   ./testdata/syntect/hard_tab.rs                     > ./testdata/syntect/hard_tab.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/long_line.rs                    > ./testdata/syntect/long_line.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/long_line_bg.rs                 > ./testdata/syntect/long_line_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --no-grid                 ./testdata/syntect/no_grid.rs                      > ./testdata/syntect/no_grid.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --tab 2                   ./testdata/syntect/tab_width_2.rs                  > ./testdata/syntect/tab_width_2.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --theme Nord              ./testdata/syntect/theme.rs                        > ./testdata/syntect/theme.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/empty_lines.rs                  > ./testdata/syntect/empty_lines.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/empty_lines_bg.rs               > ./testdata/syntect/empty_lines_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_between_text.rs            > ./testdata/syntect/wrap_between_text.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_middle_of_text.rs          > ./testdata/syntect/wrap_middle_of_text.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_middle_of_spaces.rs        > ./testdata/syntect/wrap_middle_of_spaces.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_middle_of_tab.rs           > ./testdata/syntect/wrap_middle_of_tab.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_twice.rs                   > ./testdata/syntect/wrap_twice.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --no-grid                 ./testdata/syntect/wrap_no_grid.rs                 > ./testdata/syntect/wrap_no_grid.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --theme Nord              ./testdata/syntect/wrap_theme.rs                   > ./testdata/syntect/wrap_theme.out
COLORTERM='' cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80              ./testdata/syntect/wrap_ansi256.rs                 > ./testdata/syntect/wrap_ansi256.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/wrap_middle_text_bg.rs          > ./testdata/syntect/wrap_middle_text_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/wrap_between_bg.rs              > ./testdata/syntect/wrap_between_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --wrap never              ./testdata/syntect/no_wrap_default.rs              > ./testdata/syntect/no_wrap_default.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --wrap never --no-grid    ./testdata/syntect/no_wrap_no_grid.rs              > ./testdata/syntect/no_wrap_no_grid.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --wrap never --background ./testdata/syntect/no_wrap_background.rs           > ./testdata/syntect/no_wrap_background.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/multi_line_numbers.rs           > ./testdata/syntect/multi_line_numbers.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/multi_chunks_default.rs         > ./testdata/syntect/multi_chunks_default.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --no-grid                 ./testdata/syntect/multi_chunks_no_grid.rs         > ./testdata/syntect/multi_chunks_no_grid.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/multi_chunks_bg.rs              > ./testdata/syntect/multi_chunks_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/japanese_default.rs             > ./testdata/syntect/japanese_default.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/japanese_background.rs          > ./testdata/syntect/japanese_background.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_japanese_after.rs          > ./testdata/syntect/wrap_japanese_after.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_japanese_before.rs         > ./testdata/syntect/wrap_japanese_before.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_break_wide_char.rs         > ./testdata/syntect/wrap_break_wide_char.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/wrap_break_wide_char_bg.rs      > ./testdata/syntect/wrap_break_wide_char_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_japanese_louise.rs         > ./testdata/syntect/wrap_japanese_louise.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/wrap_jp_louise_bg.rs            > ./testdata/syntect/wrap_jp_louise_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --no-grid                 ./testdata/syntect/wrap_jp_louise_no_grid.rs       > ./testdata/syntect/wrap_jp_louise_no_grid.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_emoji.rs                   > ./testdata/syntect/wrap_emoji.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_emoji_zwj.rs               > ./testdata/syntect/wrap_emoji_zwj.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/emoji.rs                        > ./testdata/syntect/emoji.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/emoji_bg.rs                     > ./testdata/syntect/emoji_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background --no-grid    ./testdata/syntect/no_grid_background.rs           > ./testdata/syntect/no_grid_background.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wide_char_region.rs             > ./testdata/syntect/wide_char_region.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/wide_char_region_bg.rs          > ./testdata/syntect/wide_char_region_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_match_at_second_line.rs    > ./testdata/syntect/wrap_match_at_second_line.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_region_accross_line.rs     > ./testdata/syntect/wrap_region_accross_line.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_region_jp_accross_line.rs  > ./testdata/syntect/wrap_region_jp_accross_line.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/wrap_match_at_second_line_bg.rs > ./testdata/syntect/wrap_match_at_second_line_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/region_at_end_of_line.rs        > ./testdata/syntect/region_at_end_of_line.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/region_at_end_of_line_bg.rs     > ./testdata/syntect/region_at_end_of_line_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/region_at_line_start.rs         > ./testdata/syntect/region_at_line_start.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_region_line_start.rs       > ./testdata/syntect/wrap_region_line_start.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_region_line_end.rs         > ./testdata/syntect/wrap_region_line_end.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/whole_line.rs                   > ./testdata/syntect/whole_line.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_whole_3_lines.rs           > ./testdata/syntect/wrap_whole_3_lines.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_3_lines_emoji.rs           > ./testdata/syntect/wrap_3_lines_emoji.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --first-only              ./testdata/syntect/first_only.rs                   > ./testdata/syntect/first_only.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --ascii-lines             ./testdata/syntect/ascii_lines_grid.rs             > ./testdata/syntect/ascii_lines_grid.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --ascii-lines --no-grid   ./testdata/syntect/ascii_lines_no_grid.rs          > ./testdata/syntect/ascii_lines_no_grid.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/multi_regions.rs                > ./testdata/syntect/multi_regions.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80 --background              ./testdata/syntect/multi_regions_bg.rs             > ./testdata/syntect/multi_regions_bg.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_between_regions.rs         > ./testdata/syntect/wrap_between_regions.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_accross_regions.rs         > ./testdata/syntect/wrap_accross_regions.out
cargo run -- '\*match to .+? line\*' -c 6 -C 6 -p syntect --term-width 80                           ./testdata/syntect/wrap_regions_japanese.rs        > ./testdata/syntect/wrap_regions_japanese.out

# Test for --list-themes
cargo run -- --list-themes -p syntect --term-width 80              > ./testdata/syntect/list_themes_default.out
cargo run -- --list-themes -p syntect --term-width 80 --no-grid    > ./testdata/syntect/list_themes_no_grid.out
cargo run -- --list-themes -p syntect --term-width 80 --background > ./testdata/syntect/list_themes_background.out

# Previews
cat ./testdata/syntect/ansi16_colors.out
cat ./testdata/syntect/ansi256_colors.out
cat ./testdata/syntect/background.out
cat ./testdata/syntect/default.out
cat ./testdata/syntect/hard_tab.out
cat ./testdata/syntect/long_line.out
cat ./testdata/syntect/no_grid.out
cat ./testdata/syntect/tab_width_2.out
cat ./testdata/syntect/theme.out
cat ./testdata/syntect/empty_lines.out
cat ./testdata/syntect/empty_lines_bg.out
cat ./testdata/syntect/wrap_between_text.out
cat ./testdata/syntect/wrap_middle_of_text.out
cat ./testdata/syntect/wrap_middle_of_spaces.out
cat ./testdata/syntect/wrap_middle_of_tab.out
cat ./testdata/syntect/wrap_twice.out
cat ./testdata/syntect/wrap_no_grid.out
cat ./testdata/syntect/wrap_theme.out
cat ./testdata/syntect/wrap_ansi256.out
cat ./testdata/syntect/wrap_middle_text_bg.out
cat ./testdata/syntect/wrap_between_bg.out
cat ./testdata/syntect/no_wrap_default.out
cat ./testdata/syntect/no_wrap_no_grid.out
cat ./testdata/syntect/no_wrap_background.out
cat ./testdata/syntect/multi_line_numbers.out
cat ./testdata/syntect/multi_chunks_default.out
cat ./testdata/syntect/multi_chunks_no_grid.out
cat ./testdata/syntect/multi_chunks_bg.out
cat ./testdata/syntect/japanese_default.out
cat ./testdata/syntect/japanese_background.out
cat ./testdata/syntect/wrap_japanese_after.out
cat ./testdata/syntect/wrap_japanese_before.out
cat ./testdata/syntect/wrap_break_wide_char.out
cat ./testdata/syntect/wrap_break_wide_char_bg.out
cat ./testdata/syntect/wrap_japanese_louise.out
cat ./testdata/syntect/wrap_jp_louise_bg.out
cat ./testdata/syntect/wrap_jp_louise_no_grid.out
cat ./testdata/syntect/wrap_emoji.out
cat ./testdata/syntect/wrap_emoji_zwj.out
cat ./testdata/syntect/emoji.out
cat ./testdata/syntect/emoji_bg.out
cat ./testdata/syntect/no_grid_background.out
cat ./testdata/syntect/wide_char_region.out
cat ./testdata/syntect/wide_char_region_bg.out
cat ./testdata/syntect/wrap_match_at_second_line.out
cat ./testdata/syntect/wrap_region_accross_line.out
cat ./testdata/syntect/wrap_region_jp_accross_line.out
cat ./testdata/syntect/wrap_match_at_second_line_bg.out
cat ./testdata/syntect/region_at_end_of_line.out
cat ./testdata/syntect/region_at_end_of_line_bg.out
cat ./testdata/syntect/region_at_line_start.out
cat ./testdata/syntect/wrap_region_line_start.out
cat ./testdata/syntect/wrap_region_line_end.out
cat ./testdata/syntect/whole_line.out
cat ./testdata/syntect/wrap_whole_3_lines.out
cat ./testdata/syntect/wrap_3_lines_emoji.out
cat ./testdata/syntect/first_only.out
cat ./testdata/syntect/ascii_lines_grid.out
cat ./testdata/syntect/ascii_lines_no_grid.out
cat ./testdata/syntect/multi_regions.out
cat ./testdata/syntect/multi_regions_bg.out
cat ./testdata/syntect/wrap_between_regions.out
cat ./testdata/syntect/wrap_accross_regions.out
cat ./testdata/syntect/wrap_regions_japanese.out

cat ./testdata/syntect/list_themes_default.out
cat ./testdata/syntect/list_themes_no_grid.out
cat ./testdata/syntect/list_themes_background.out
