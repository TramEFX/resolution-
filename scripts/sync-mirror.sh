#!/usr/bin/env bash
# Sync the private engine mirror (TramEFX/resolution-filmcraft) to upstream FilmCraft and cut a
# sync point. See docs/UPSTREAM.md.
#
#   scripts/sync-mirror.sh [upstream-ref] [--apply]
#
# upstream-ref  what to sync to (default: upstream/main; a release tag like v0.5.0 is better)
# --apply       also move the engine `rev` in Cargo.toml and run `cargo test`
#
# Steps: fast-forward the mirror's `upstream` branch, rebase our `resolution` patch stack onto it,
# push both, and push the result as `sync/fc-YYYYMMDD` (which starts mirror-ci on Linux).
set -euo pipefail

root="$(cd "$(dirname "$0")/.." && pwd)"
mirror_url="https://github.com/TramEFX/resolution-filmcraft"
upstream_url="https://github.com/storytold/filmcraft"
dir="${MIRROR_DIR:-$root/target/mirror/resolution-filmcraft}"

ref="upstream/main"
apply=0
for a in "$@"; do
  case "$a" in
    --apply) apply=1 ;;
    *) ref="$a" ;;
  esac
done

if [ ! -d "$dir/.git" ]; then
  mkdir -p "$(dirname "$dir")"
  git clone --quiet "$mirror_url" "$dir"
fi
git -C "$dir" remote get-url upstream >/dev/null 2>&1 || git -C "$dir" remote add upstream "$upstream_url"
git -C "$dir" fetch --quiet origin
git -C "$dir" fetch --quiet --tags upstream

new_base="$(git -C "$dir" rev-parse "$ref^{commit}")"
old_base="$(git -C "$dir" rev-parse origin/upstream)"
if [ "$new_base" = "$old_base" ]; then
  echo "mirror is already at $ref ($(git -C "$dir" rev-parse --short "$new_base")); nothing to sync"
  exit 0
fi
if ! git -C "$dir" merge-base --is-ancestor "$old_base" "$new_base"; then
  echo "error: $ref is not a fast-forward of the mirror's upstream branch (upstream rewrote history?)" >&2
  exit 1
fi
echo "upstream: $(git -C "$dir" rev-parse --short "$old_base") -> $(git -C "$dir" rev-parse --short "$new_base") ($(git -C "$dir" rev-list --count "$old_base..$new_base") commits)"

git -C "$dir" checkout --quiet -B resolution origin/resolution
if ! git -C "$dir" rebase --quiet --onto "$new_base" "$old_base" resolution; then
  # Upstream changed a workflow we removed: keep it removed. Anything else needs a human.
  while [ -d "$dir/.git/rebase-merge" ] || [ -d "$dir/.git/rebase-apply" ]; do
    conflicts="$(git -C "$dir" diff --name-only --diff-filter=U)"
    others="$(printf '%s\n' "$conflicts" | grep -v '^\.github/workflows/' | grep -v '^$' || true)"
    if [ -n "$others" ] || printf '%s\n' "$conflicts" | grep -qx '.github/workflows/mirror-ci.yml'; then
      echo "error: rebase conflicts need a human, in $dir:" >&2
      printf '  %s\n' $conflicts >&2
      echo "resolve, 'git rebase --continue', then re-run this script" >&2
      exit 1
    fi
    printf '%s\n' "$conflicts" | xargs -r git -C "$dir" rm --quiet
    GIT_EDITOR=true git -C "$dir" rebase --continue >/dev/null || true
  done
fi

echo "patch stack:"
git -C "$dir" log --oneline "$new_base..resolution" | sed 's/^/  /'

sync_branch="sync/fc-$(date -u +%Y%m%d)"
if git -C "$dir" ls-remote --exit-code --heads origin "$sync_branch" >/dev/null; then
  sync_branch="$sync_branch-$(git -C "$dir" rev-parse --short "$new_base")"
fi
git -C "$dir" push --quiet origin "$new_base:refs/heads/upstream"
git -C "$dir" push --quiet --force-with-lease=resolution origin resolution
git -C "$dir" push --quiet origin "resolution:refs/heads/$sync_branch"
rev="$(git -C "$dir" rev-parse resolution)"
echo "sync point: $sync_branch = $rev (mirror-ci is now running on Linux)"

if [ "$apply" = 1 ]; then
  sed -i.bak -E "s|^(filmcraft-engine = \{ git = \"[^\"]*\", rev = \")[0-9a-f]+(\" \})|\1$rev\2|" "$root/Cargo.toml"
  sed -i.bak -E "s|^# a sync point's commit \(branch sync/[^ ]+ |# a sync point's commit (branch $sync_branch |" "$root/Cargo.toml"
  rm -f "$root/Cargo.toml.bak"
  (cd "$root" && cargo update -p filmcraft-engine && cargo test)
  echo "Cargo.toml now pins $rev. Review the command-id diff (scripts/upstream-status.sh) and commit."
else
  echo "next: wait for mirror-ci, then re-run with --apply (or set rev = \"$rev\" in Cargo.toml)"
fi
