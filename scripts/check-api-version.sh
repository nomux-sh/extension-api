#!/usr/bin/env bash
# Fails if a constant of an SDK is not equal to spec/api-version.toml, or if
# the newest row of the version table of the specification is a different
# version. Run the script from any directory in the repository.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

value() {
  sed -n "s/^$1 = \"\(.*\)\"$/\1/p" spec/api-version.toml
}

# Prints the version of the constant `$2` in the file `$1`: the numbers that
# follow the name of the constant, in the sequence major, minor, patch.
constant() {
  grep -A 4 -E "$2" "$1" | grep -oE '(major|minor|patch|Major|Minor|Patch): [0-9]+' |
    head -n 3 | grep -oE '[0-9]+' | paste -sd .
}

current="$(value current)"
minimum="$(value minimum)"
failures=0
compare() {
  local name="$1" found="$2" expected="$3"
  if [ "$found" != "$expected" ]; then
    echo "error: $name is $found, but spec/api-version.toml has $expected" >&2
    failures=$((failures + 1))
  fi
}

rust=crates/nomux-extension/src/api_version.rs
typescript=sdk/typescript/src/api-version.ts
go=sdk/go/version.go
compare "ApiVersion::CURRENT" "$(constant "$rust" 'const CURRENT')" "$current"
compare "ApiVersion::MINIMUM" "$(constant "$rust" 'const MINIMUM')" "$minimum"
compare "API_VERSION" "$(constant "$typescript" 'const API_VERSION')" "$current"
compare "MINIMUM_API_VERSION" "$(constant "$typescript" 'const MINIMUM_API_VERSION')" "$minimum"
compare "APIVersion()" "$(constant "$go" 'func APIVersion\(\)')" "$current"
compare "MinimumAPIVersion()" "$(constant "$go" 'func MinimumAPIVersion\(\)')" "$minimum"

# The newest row is the last row of the table under "### Versions".
newest="$(sed -n '/^### Versions$/,/^#/p' spec/extension-protocol.md |
  grep -E '^\| [0-9]+\.[0-9]+\.[0-9]+ \|' | tail -n 1 | cut -d '|' -f 2 | tr -d ' ')"
compare "the newest version of the table in spec/extension-protocol.md" "$newest" "$current"

if [ "$failures" -ne 0 ]; then
  exit 1
fi
echo "the extension API version is $current, the minimum is $minimum"
