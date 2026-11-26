#!/usr/bin/env bash
# Installs the three echo examples in a released nomux and calls each of
# them.
#
# Build the examples first: `cargo build -p nomux-echo`, `npm run build
# --workspaces` and the `go build` command of examples/echo-go/README.md. The
# `nomux` program on the PATH is the host. The script uses a temporary HOME,
# thus it does not change the nomux of the user, and it stops the daemon at
# the end. Run it from any directory in the repository.
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

work="$(mktemp -d)"
export HOME="$work/home"
export XDG_RUNTIME_DIR="$work/run"
mkdir -p "$HOME" "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
stop() {
  nomux daemon stop > /dev/null 2>&1 || true
  rm -rf "$work"
}
trap stop EXIT

# The package of an extension: its manifest and the files of its program.
package() {
  local name="$1" example="$2"
  shift 2
  mkdir -p "$work/$name"
  cp "$example/extension.toml" "$work/$name/"
  for file in "$@"; do
    mkdir -p "$work/$name/$(dirname "$file")"
    cp -R "$example/$file" "$work/$name/$file"
  done
}
package rust examples/echo
mkdir -p "$work/rust/bin"
cp target/debug/nomux-echo "$work/rust/bin/nomux-echo"
package typescript examples/echo-typescript bin/nomux-echo dist/main.js package.json
package go examples/echo-go bin/nomux-echo

# The payload is the protobuf message `Text` with the text "hi".
printf '\x0a\x02hi' > "$work/payload"

nomux --version
failures=0
for name in rust typescript go; do
  id="$(sed -n 's/^id = "\(.*\)"$/\1/p' "$work/$name/extension.toml")"
  nomux ext install --grant-all "$work/$name"
  # `nomux ext call` 0.4.0 exits with the status 0 also when the extension
  # fails. Thus the script compares the result with the payload.
  nomux ext call "$id" echo --payload "@$work/payload" > "$work/result" || true
  if cmp --silent "$work/payload" "$work/result"; then
    echo "ok    $id"
  else
    failures=$((failures + 1))
    echo "FAIL  $id: the result of echo is not the payload"
    nomux ext ls
  fi
done

if [ "$failures" -ne 0 ]; then
  echo "$failures extension(s) failed with the released host" >&2
  exit 1
fi
echo "each extension runs on the released host"
