#!/usr/bin/bash
set -euo pipefail

MEMBERS="$(grep Cargo.toml -Pe members --color=never | grep -Poe "\[.+\]" --color=never | jq .[])" # looks close enough to JSON to work
CARGO_SUBCOMMAND="${1:-publish}"

for member in $MEMBERS; do
  member="$(echo "$member" | cut -d\" -f2)"
  if ! cargo "$CARGO_SUBCOMMAND" -p "$member"; then
    echo "[WARN] failed to $CARGO_SUBCOMMAND $member"
  fi
done
