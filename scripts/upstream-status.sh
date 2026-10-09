#!/usr/bin/env bash
# What changed upstream since our pin: commits, ROADMAP log, and FilmCraft command ids.
# Usage: scripts/upstream-status.sh [upstream-ref]   (default: origin/main of storytold/filmcraft)
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
pin="$(sed -n 's/^filmcraft-engine = .*rev = "\([0-9a-f]*\)".*/\1/p' "$root/Cargo.toml")"
url="$(sed -n 's/^filmcraft-engine = { git = "\([^"]*\)".*/\1/p' "$root/Cargo.toml")"
cache="${FILMCRAFT_CACHE:-$root/target/upstream/filmcraft}"
ref="${1:-origin/main}"

if [ ! -d "$cache/.git" ]; then
  mkdir -p "$(dirname "$cache")"
  git clone --quiet --filter=blob:none https://github.com/storytold/filmcraft "$cache"
fi
git -C "$cache" fetch --quiet --tags origin

echo "pinned:   $pin  ($url)"
echo "upstream: $(git -C "$cache" rev-parse --short "$ref")  $(git -C "$cache" log -1 --format=%ci "$ref")"
echo "commits since pin: $(git -C "$cache" rev-list --count "$pin..$ref")"
echo "latest tag: $(git -C "$cache" describe --tags --abbrev=0 "$ref" 2>/dev/null || echo none)"

echo
echo "== engine-relevant churn since pin (files changed) =="
git -C "$cache" diff --stat "$pin" "$ref" -- crates/engine crates/project crates/format crates/edit crates/platform | tail -n 15

ids() { git -C "$cache" show "$1:crates/engine/src/commands.rs" | grep -oE '(cmd|query)!\(\s*"[A-Za-z0-9_.]+"' | grep -oE '"[^"]+"' | tr -d '"' | sort -u; }
echo
echo "== command ids (commands.rs) removed (-) / added (+) since pin =="
diff <(ids "$pin") <(ids "$ref") | grep -E '^[<>]' | sed 's/^</-/; s/^>/+/' || echo "(none)"

echo
echo "== ROADMAP.md changes since pin =="
git -C "$cache" diff "$pin" "$ref" -- ROADMAP.md | grep -E '^[+-][^+-]' | head -n 60 || echo "(none)"
