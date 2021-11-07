#!/bin/bash

set -e -o pipefail

if [ ! -d '.git' ]; then
    echo 'This script must be run at root of the repository' >&2
    exit 1
fi

if [[ "$1" == "" || "$1" == "-h" || "$1" == "--help" ]]; then
    echo 'Usage: update.sh {tag name}' >&2
    exit 1
fi


case "$OSTYPE" in
    darwin*|freebsd*)
        _sed() {
            sed -i '' "$@"
        }
    ;;
    *)
        _sed() {
            sed -i "$@"
        }
    ;;
esac

cd ./HomebrewFormula

VERSION="$1"

# \d was not available
if [[ ! "$VERSION" =~ ^v[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
    echo 'Version number in first argument must match to ^v\d+\.\d+\.\d+$ like v1.2.3' >&2
    exit 1
fi

echo "Update formula to version ${VERSION}"

function _update() {
    local triple mark zip url sha

    triple="$1"
    mark="$2"

    # macOS x86_64
    zip="hgrep-${VERSION}-${triple}.zip"
    url="https://github.com/rhysd/hgrep/releases/download/${VERSION}/${zip}"

    echo "Downloading ${zip}..."
    curl -f -LO "$url"
    sha="$(shasum -a 256 "$zip" | cut -f 1 -d ' ')"
    echo "${zip} sha256: ${sha}"
    _sed -E "s/    sha256 '[0-9a-f]*' # ${mark}/    sha256 '${sha}' # ${mark}/" hgrep.rb
}

_update 'x86_64-apple-darwin' 'mac_x86_64'
_update 'aarch64-apple-darwin' 'mac_aarch64'
_update 'x86_64-unknown-linux-gnu' 'linux'

echo "Clean up zip files"
rm -rf ./*.zip

echo 'Done.'
