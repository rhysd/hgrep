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


# macOS x86_64
MAC_ZIP_X86_64="hgrep-${VERSION}-x86_64-apple-darwin.zip"
MAC_URL_X86_64="https://github.com/rhysd/hgrep/releases/download/${VERSION}/${MAC_ZIP_X86_64}"

echo "Downloading ${MAC_ZIP_X86_64}..."
curl -LO "$MAC_URL_X86_64"
MAC_SHA_X86_64="$(shasum -a 256 "$MAC_ZIP_X86_64" | cut -f 1 -d ' ')"
echo "Mac x86_64 sha256: ${MAC_SHA_X86_64}"
_sed -E "s/    sha256 '[0-9a-f]*' # mac_x86_64/    sha256 '${MAC_SHA_X86_64}' # mac_x86_64/" hgrep.rb


# macOS aarch64
MAC_ZIP_AARCH64="hgrep-${VERSION}-aarch64-darwin.zip"
MAC_URL_AARCH64="https://github.com/rhysd/hgrep/releases/download/${VERSION}/${MAC_ZIP_AARCH64}"

echo "Downloading ${MAC_ZIP_AARCH64}..."
curl -LO "$MAC_URL_AARCH64"
MAC_SHA_AARCH64="$(shasum -a 256 "$MAC_ZIP_AARCH64" | cut -f 1 -d ' ')"
echo "Mac aarch64 sha256: ${MAC_SHA_AARCH64}"
_sed -E "s/    sha256 '[0-9a-f]*' # mac_aarch64/    sha256 '${MAC_SHA_AARCH64}' # mac_aarch64/" hgrep.rb


# Linux x86_64
LINUX_ZIP="hgrep-${VERSION}-x86_64-unknown-linux-gnu.zip"
LINUX_URL="https://github.com/rhysd/hgrep/releases/download/${VERSION}/${LINUX_ZIP}"

echo "Downloading ${LINUX_ZIP}..."
curl -LO "$LINUX_URL"
LINUX_SHA="$(shasum -a 256 "$LINUX_ZIP" | cut -f 1 -d ' ')"
echo "Linux sha256: ${LINUX_SHA}"
_sed -E "s/    sha256 '[0-9a-f]*' # linux/    sha256 '${LINUX_SHA}' # linux/" hgrep.rb

echo "Version: ${VERSION}"
_sed -E "s/  version '[0-9][0-9]*\.[0-9][0-9]*\.[0-9][0-9]*'/  version '${VERSION#v}'/" hgrep.rb

echo "Clean up zip files"
rm -rf ./*.zip

echo 'Done.'
