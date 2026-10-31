#!/usr/bin/env bash
# Release version from Conventional Commits (type(scope): summary) since the latest final
# release tag vX.Y.Z; merge commits are skipped.
#   feat                          -> minor
#   any other type                -> patch
#   "type!:" or "BREAKING CHANGE:" -> major (minor while the version is 0.x)
#
#   next-version.sh                 prints the next version, e.g. 0.6.0
#   next-version.sh --check RANGE   fails if a commit in RANGE is not a Conventional Commit
set -euo pipefail

types='feat|fix|perf|refactor|docs|test|build|ci|chore|style|revert'
conventional="^($types)(\([^)]+\))?!?: .+"

if [ "${1:-}" = "--check" ]; then
  bad=$(git log --no-merges --format='%h %s' "$2" | grep -vE "^[0-9a-f]+ ${conventional#^}" || true)
  if [ -n "$bad" ]; then
    echo "::error::Commit messages must be Conventional Commits (type(scope): summary, types: ${types//|/, }):"
    echo "$bad"
    exit 1
  fi
  exit 0
fi

# The newest final tag: on main it is an ancestor; on dev its commits are, which is what the
# range below needs.
last=$(git tag -l 'v*' | { grep -E '^v[0-9]+\.[0-9]+\.[0-9]+$' || true; } | sort -V | tail -n1)
if [ -n "$last" ]; then
  range="$last..HEAD"
else
  last=v0.0.0
  range=HEAD
fi

subjects=$(git log --no-merges --format='%s' "$range")
if [ -z "$subjects" ]; then
  echo "::error::No commits since $last: nothing to release." >&2
  exit 1
fi
bodies=$(git log --no-merges --format='%b' "$range")

IFS=. read -r major minor patch <<< "${last#v}"
if grep -qE "^[a-z]+(\([^)]+\))?!: " <<< "$subjects" || grep -qE '^BREAKING[ -]CHANGE:' <<< "$bodies"; then
  bump=breaking
  if [ "$major" -eq 0 ]; then minor=$((minor + 1)); patch=0; else major=$((major + 1)); minor=0; patch=0; fi
elif grep -qE '^feat(\([^)]+\))?: ' <<< "$subjects"; then
  bump=feat
  minor=$((minor + 1)); patch=0
else
  bump=patch
  patch=$((patch + 1))
fi

echo "$last + $(wc -l <<< "$subjects") commit(s), $bump -> $major.$minor.$patch" >&2
echo "$major.$minor.$patch"
