#!/bin/sh
# bloop:managed checks
#
# The one gate for this project. Dev machines and CI call this same file.
# The `check` job of .github/workflows/ci.yml runs this file and nothing else.
#
#   ./checks.sh         format + lint + unit/integration   every slice, must run offline
#   ./checks.sh --e2e   same as above: there is no E2E smoke test, because no test
#                       may send an authenticated request or an order to the live venue
#
# Not here: the fuzz job (nightly + cargo-fuzz, see ci.yml) and the dependency audit
# (needs the network, see .github/workflows/audit.yml).
set -eu

MODE="${1:-}"

step() { printf '\n==> %s\n' "$1"; }

step "format"
cargo fmt --check

step "lint"
cargo clippy --all-targets -- -D warnings
cargo clippy --all-targets --features examples -- -D warnings

# Rust type checks inside clippy above; there is no separate typecheck step.
# Boundary rules (ARCHITECTURE.md, ADR-0001) are checked by review: no Rust tool is wired in.

step "unit + integration"
PLAIN=$(mktemp)
EXAMPLES=$(mktemp)
trap 'rm -f "$PLAIN" "$EXAMPLES"' EXIT
# Kept for the count step; a failing run still stops the gate here.
cargo test > "$PLAIN" 2>&1 || { cat "$PLAIN"; exit 1; }
cat "$PLAIN"
cargo test --features examples > "$EXAMPLES" 2>&1 || { cat "$EXAMPLES"; exit 1; }
cat "$EXAMPLES"

# docs/STATUS.md writes the passing count of each run by hand: it has to match what ran.
step "test count"
counted() { awk '/^test result:/ { n += $4 } END { print n + 0 }' "$1"; }
compare() {
  row=$1 written=$2 run=$3
  if [ -z "$written" ]; then
    printf 'docs/STATUS.md: no test count found in the row %s\n' "$row" >&2
    exit 1
  fi
  if [ "$written" != "$(counted "$run")" ]; then
    printf 'docs/STATUS.md: the row %s says %s passed, cargo test counted %s\n' \
      "$row" "$written" "$(counted "$run")" >&2
    exit 1
  fi
  printf '%s: %s passed, as written\n' "$row" "$written"
}
compare '`cargo test --features examples`' \
  "$(sed -nE 's/^\| `cargo test --features examples` \| ([0-9]+) passed.*/\1/p' docs/STATUS.md)" "$EXAMPLES"
compare '`cargo test`' \
  "$(sed -nE 's/^\| `cargo test` \(no feature.*\| The same ([0-9]+) \|.*/\1/p' docs/STATUS.md)" "$PLAIN"

# A shortcut marker has to be complete: a ceiling and a trigger to revisit, comma separated.
# The marker may wrap: the comment lines that follow it are read as part of it.
step "shortcut marker"
BAD=$(grep -rlE '(#|//|/\*) ?shortcut:' . \
  --exclude-dir=.git --exclude-dir=target --exclude-dir=.worktrees 2>/dev/null \
  | while read -r f; do
      awk -v f="$f" '
        /(#|\/\/|\/\*) ?shortcut:/ { if (m) check(); m = 1; t = $0; n = FNR; next }
        m && /^[[:space:]]*(#|\/\/|\*)/ { t = t " " $0; next }
        m { check(); m = 0 }
        END { if (m) check() }
        function check() { if (t !~ /shortcut:[^,]+,/) print f ":" n ": " t }
      ' "$f"
    done || :)
if [ -n "$BAD" ]; then
  printf 'shortcut markers with no trigger to revisit:\n%s\n' "$BAD" >&2
  printf 'the correct shape is  shortcut: <ceiling>, <when to revisit>\n' >&2
  exit 1
fi

if [ "$MODE" = "--e2e" ]; then
  step "e2e smoke"
  printf 'no E2E step: tests never contact the live QFEX venue\n'
fi

printf '\nall checks passed\n'
# bloop:end checks
