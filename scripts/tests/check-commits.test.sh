#!/usr/bin/env bash
# Tests for scripts/check-commits.sh (spec US6). Run: bash scripts/tests/check-commits.test.sh
set -u

script="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)/check-commits.sh"
failures=0

# expect <exit-code> <description> <subject>
expect() {
  local want="$1" what="$2" subject="$3" got
  printf '%s\n' "$subject" | bash "$script" --stdin >/dev/null 2>&1
  got=$?
  if [ "$got" -ne "$want" ]; then
    echo "FAIL: $what (subject: '$subject'): expected exit $want, got $got"
    failures=$((failures + 1))
  else
    echo "ok:   $what"
  fi
}

expect 0 "feat with scope"                 "feat(ui): add signal view"
expect 0 "fix with nested scope"           "fix(drivers/logic16): handle short read"
expect 0 "docs without scope"              "docs: update contributor guide"
expect 0 "chore without scope"             "chore: add .gitignore"
expect 0 "breaking marker"                 "feat(core)!: change error codes"
expect 0 "merge commits are ignored"       "Merge branch 'main' into 001-x"
expect 1 "free-form message"               "Update stuff"
expect 1 "feat without scope"              "feat: missing scope for code change"
expect 1 "unknown type"                    "wip(core): something"
expect 1 "missing colon space"             "feat(core):no space"
expect 1 "non-English text"                "feat(ui): додати кнопку"
expect 1 "empty description"               "fix(core): "

# Several subjects: one bad subject makes the run fail.
printf 'feat(ui): ok\nUpdate stuff\n' | bash "$script" --stdin >/dev/null 2>&1
if [ $? -ne 1 ]; then echo "FAIL: mixed input must fail"; failures=$((failures + 1)); else echo "ok:   mixed input fails"; fi

# The offender is named in the output.
out="$(printf 'Update stuff\n' | bash "$script" --stdin 2>&1)"
case "$out" in
  *"Update stuff"*) echo "ok:   offender is reported" ;;
  *) echo "FAIL: offender not reported: $out"; failures=$((failures + 1)) ;;
esac

if [ "$failures" -ne 0 ]; then
  echo "$failures test(s) failed"
  exit 1
fi
echo "all tests passed"
