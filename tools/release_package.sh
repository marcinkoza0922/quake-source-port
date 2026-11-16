#!/bin/sh
# Package a release binary: tools/release_package.sh <target-name> <path-to-binary>
# Writes quake-source-port-<ref>-<target-name>.tar.gz and prints its name.
set -eu
name="$1"; bin="$2"
ref="$(printf %s "${GITHUB_REF_NAME:-dev}" | tr / -)"
d="quake-source-port-$ref-$name"
rm -rf "$d"; mkdir -p "$d/docs"
cp "$bin" README.md LICENSE "$d/"
cp docs/MANUAL.md "$d/docs/"
tar czf "$d.tar.gz" "$d"
echo "$d.tar.gz"
