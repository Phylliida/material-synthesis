#!/usr/bin/env bash
# Verify the protolith core under the tactus Lean backend.
#
# Usage:
#   ./check.sh                        # verify the whole library
#   ./check.sh --verify-module slab   # verify one module
#
# Requires the tactus verus binary at ../../tactus/source/target-verus/release/verus
# (see tactus-tutorial/chapters/00-setup).
#
# The binary (src/main.rs) is the deliberately-unverified I/O shell; only the
# library core is checked. Build/run the app with plain `cargo run --release`.
set -euo pipefail
HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
VERUS="$HERE/../../tactus/source/target-verus/release/verus"

if [[ ! -x "$VERUS" ]]; then
  echo "error: tactus verus binary not found at $VERUS" >&2
  echo "build it with: cd ../../tactus/source && vargo build --release" >&2
  exit 1
fi

exec "$VERUS" --lean-backend --crate-type=lib "$HERE/src/lib.rs" "$@"
