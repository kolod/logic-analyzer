#!/usr/bin/env bash
# Reports commit subjects that do not follow Conventional Commits with a component scope
# (see CONTRIBUTING.md). Dependency-free; exit status 1 if any subject does not conform.
#
# Usage:
#   scripts/check-commits.sh <git-range>      e.g. origin/main..HEAD
#   some-command | scripts/check-commits.sh --stdin   (one subject per line)
set -u

if [ "$#" -ne 1 ]; then
  echo "usage: $0 <git-range> | --stdin" >&2
  exit 2
fi

if [ "$1" = "--stdin" ]; then
  subjects="$(cat)"
else
  subjects="$(git log --format=%s "$1")" || exit 2
fi

# Types whose scope is optional; every other type requires "(scope)".
optional_scope='docs|chore|build|ci|revert'
required_scope='feat|fix|perf|refactor|test|style'
scope='\([a-z0-9][a-z0-9/_.-]*\)'

bad=0
while IFS= read -r subject; do
  [ -z "$subject" ] && continue
  case "$subject" in Merge\ *) continue ;; esac

  ok=1
  # Type, optional scope rules, optional "!" and a non-empty description.
  if ! printf '%s' "$subject" | grep -Eq "^(($optional_scope)($scope)?|($required_scope)$scope)!?: [^ ].*"; then
    ok=0
  fi
  # Commit messages are English: reject anything outside printable ASCII.
  if printf '%s' "$subject" | LC_ALL=C grep -q '[^ -~]'; then
    ok=0
  fi

  if [ "$ok" -eq 0 ]; then
    echo "NOT CONFORMING: $subject"
    bad=$((bad + 1))
  fi
done <<EOF
$subjects
EOF

if [ "$bad" -ne 0 ]; then
  echo "$bad commit subject(s) do not follow Conventional Commits with a scope (see CONTRIBUTING.md)." >&2
  exit 1
fi
echo "All commit subjects conform."
