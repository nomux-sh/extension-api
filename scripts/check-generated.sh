#!/usr/bin/env bash
# Fails if the generated code in the repository is stale.
#
# The script runs `buf generate` and then compares the result with the
# committed files: the Rust code, the TypeScript code and the Go code. Run
# `npm ci` first, for the TypeScript plugin. The Go plugin needs Go. Run the
# script from any directory in the repository.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

generated_dirs=("crates/nomux-extension/src/generated" "sdk/typescript/src/generated" "sdk/go/internal/wire")

buf generate

stale="$(git status --porcelain -- "${generated_dirs[@]}")"
if [ -n "$stale" ]; then
  echo "error: generated code is stale. Run 'buf generate' and commit the result." >&2
  printf '%s\n' "$stale" >&2
  git --no-pager diff -- "${generated_dirs[@]}" >&2
  exit 1
fi

echo "generated code is up to date"
