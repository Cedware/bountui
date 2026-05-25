#!/bin/bash
set -euo pipefail

PKGBUILD="$1"
PKGVER="$2"
PKGREL="$3"
SOURCE_URL="$4"
SHA256SUMS="$5"

sed -i "s/pkgver=.*/pkgver=$PKGVER/" "$PKGBUILD"
sed -i "s/pkgrel=.*/pkgrel=$PKGREL/" "$PKGBUILD"

if [ -n "$SOURCE_URL" ]; then
    sed -i "s|source=.*|source=(\"$SOURCE_URL\")|" "$PKGBUILD"
else
    sed -i "s/source=.*/source=()/" "$PKGBUILD"
fi

if [ -n "$SHA256SUMS" ]; then
    sed -i "s|sha256sums=.*|sha256sums=('$SHA256SUMS')|" "$PKGBUILD"
else
    sed -i "s/sha256sums=.*/sha256sums=()/" "$PKGBUILD"
fi
