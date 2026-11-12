#!/usr/bin/env bash
# Proves the lint that keeps HTTP clients inside `tandem-outbound` (BR-19) still fails the build:
# plants a client construction in a crate that is not exempt, expects clippy to reject it, then
# restores the tree.
set -euo pipefail
cd "$(dirname "$0")/../.."

lib=crates/credentials/src/lib.rs
probe=crates/credentials/src/outbound_lint_probe.rs
backup="$(mktemp)"
cp "$lib" "$backup"
cleanup() {
  cp "$backup" "$lib"
  rm -f "$probe" "$backup"
}
trap cleanup EXIT

cat > "$probe" <<'RUST'
pub fn bypass() -> reqwest::Client {
    reqwest::Client::new()
}
RUST
printf '\nmod outbound_lint_probe;\n' >> "$lib"

if output="$(cargo clippy -p tandem-credentials -- -D warnings 2>&1)"; then
  echo "FAIL: building an HTTP client outside tandem-outbound was not rejected" >&2
  exit 1
fi
if ! grep -q "disallowed_methods" <<<"$output"; then
  echo "FAIL: clippy failed, but not on the disallowed-methods rule:" >&2
  echo "$output" >&2
  exit 1
fi
echo "ok: the outbound lint rejects a client built outside tandem-outbound"
