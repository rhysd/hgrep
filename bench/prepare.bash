#!/bin/bash

set -e -o pipefail

if [[ "$(cat Cargo.toml)" != *'name = "hgrep-bench"'* ]]; then
    echo 'This script must be run at hgrep/behch/ directory' 1>&2
    exit 1
fi

echo 'Generating ./rust_releases.md'
curl -Ls 'https://raw.githubusercontent.com/rust-lang/rust/master/RELEASES.md' -o rust_releases.md

echo 'Generating ./node_modules'
npm install
