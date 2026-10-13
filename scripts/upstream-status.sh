#!/usr/bin/env bash
# What changed upstream since our pinned sync point: commits, engine churn, command ids, ROADMAP.
# Usage: scripts/upstream-status.sh [upstream-ref]   (default: upstream/main)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
dir="${MIRROR_DIR:-$root/target/mirror/resolution-filmcraft}"
pin="$(sed -n 's/^filmcraft-engine = .*rev = "\([0-9a-f]*\)".*/\1/p' "$root/Cargo.toml")"
ref="${1:-upstream/main}"

if [ ! -d "$dir/.git" ]; then
  mkdir -p "$(dirname "$dir")"
  git clone --quiet https://github.com/TramEFX/resolution-filmcraft "$dir"
fi
git -C "$dir" remote get-url upstream >/dev/null 2>&1 || git -C "$dir" remote add upstream https://github.com/storytold/filmcraft
git -C "$dir" fetch --quiet origin
git -C "$dir" fetch --quiet --tags upstream

base="$(git -C "$dir" merge-base "$pin" "$ref")"
echo "pinned:      $(git -C "$dir" rev-parse --short "$pin") (upstream base $(git -C "$dir" rev-parse --short "$base"), $(git -C "$dir" log -1 --format=%cs "$base"))"
echo "upstream:    $(git -C "$dir" rev-parse --short "$ref") ($(git -C "$dir" log -1 --format=%cs "$ref"))"
echo "behind by:   $(git -C "$dir" rev-list --count "$base..$ref") commits"
echo "latest tag:  $(git -C "$dir" describe --tags --abbrev=0 "$ref" 2>/dev/null || echo none)"
echo "patch stack: $(git -C "$dir" rev-list --count "$base..$pin") commits"

echo
echo "== engine-relevant churn (files changed) =="
git -C "$dir" diff --stat "$base" "$ref" -- crates/engine crates/project crates/format crates/edit crates/platform | tail -n 15

# Joined into one line first: rustfmt puts a long `cmd!(` and its id on separate lines (LOG B-018).
ids() { git -C "$dir" show "$1:crates/engine/src/commands.rs" | tr '\n' ' ' | grep -oE '(cmd|query)!\(\s*"[A-Za-z0-9_.]+"' | grep -oE '"[^"]+"' | tr -d '"' | sort -u; }
echo
echo "== command ids in commands.rs removed (-) / added (+) =="
# Captured first: under pipefail, diff's "differences found" exit status would also print "(none)".
changes="$(diff <(ids "$base") <(ids "$ref") | grep -E '^[<>]' | sed 's/^</-/; s/^>/+/' || true)"
echo "${changes:-(none)}"

echo
echo "== ROADMAP.md changes =="
roadmap="$(git -C "$dir" diff "$base" "$ref" -- ROADMAP.md | grep -E '^[+-][^+-]' || true)"
if [ -n "$roadmap" ]; then head -n 60 <<<"$roadmap"; else echo "(none)"; fi
