#!/usr/bin/env bash
# Bring nuphos/nuphos main into ThemeXpert's fork.
#
#   main        = exact mirror of upstream main (fast-forward only, never commit here)
#   themexpert  = our default branch: upstream + our customizations
#
# Usage: scripts/themexpert/sync-upstream.sh
set -euo pipefail

cd "$(git rev-parse --show-toplevel)"

if ! git diff --quiet || ! git diff --cached --quiet; then
  echo "Working tree has changes. Commit or stash them first." >&2
  exit 1
fi

git remote get-url upstream >/dev/null 2>&1 ||
  git remote add upstream https://github.com/nuphos/nuphos.git

start=$(git branch --show-current)
git fetch upstream main
git fetch origin

# 1. Mirror upstream into main. Fails if someone committed to our main.
git switch main
git merge --ff-only upstream/main
git push origin main

# 2. Merge it into our branch.
git switch themexpert
git merge --ff-only origin/themexpert
if ! git merge --no-edit main; then
  echo
  echo "Merge conflicts. Resolve them (see THEMEXPERT.md), then:" >&2
  echo "  git add -A && git commit && git push origin themexpert" >&2
  exit 1
fi
git push origin themexpert

[ -n "$start" ] && [ "$start" != themexpert ] && git switch "$start"
echo "themexpert is up to date with upstream $(git rev-parse --short upstream/main)."
