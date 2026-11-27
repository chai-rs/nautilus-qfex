#!/bin/sh
# bloop:managed inject-rules
# SessionStart hook: prints the project's rules so a new session starts with them.
# Never touches the network.
set -eu
ROOT=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
[ -f "$ROOT/AGENTS.md" ] && cat "$ROOT/AGENTS.md"
printf '\nArchitecture: read ARCHITECTURE.md before changing code.\n'
exit 0
# bloop:end inject-rules
