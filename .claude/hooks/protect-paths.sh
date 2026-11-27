#!/bin/sh
# bloop:managed protect-paths
# PreToolUse hook: blocks Write/Edit on paths that need a human.
# Reads the tool call as JSON on stdin. Never touches the network.
set -eu
INPUT=$(cat)
FILE=$(printf '%s' "$INPUT" | sed -n 's/.*"file_path"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | head -n 1)
case "$FILE" in
  */.bloop/config.json|.bloop/config.json)
    printf 'blocked: %s is protected; change it through /bloop:guardrails or ask a human\n' "$FILE" >&2
    exit 2 ;;
  */docs/adr/*|docs/adr/*)
    if [ -f "$FILE" ]; then
      printf 'blocked: %s is an ADR; ADRs are append-only, write a new one instead\n' "$FILE" >&2
      exit 2
    fi ;;
esac
exit 0
# bloop:end protect-paths
