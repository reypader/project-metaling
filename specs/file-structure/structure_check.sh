#!/usr/bin/env bash
# REF-03: report files that break the file-structure rules (specs/file-structure/plan.md).
#   multi-type: a non-`_dto.rs` source file declaring >1 non-private top-level struct/enum/trait
#   inline-tests: a source file with an inline `mod tests {` block
# Exit 1 if anything outside the allowlist is reported.
set -uo pipefail
cd "$(git rev-parse --show-toplevel)"
allow='^(crates/ro-packets/src/outgoing/encoder\.rs)$'   # rule 5: kept as is
bad=0
for f in $(git ls-files -co --exclude-standard 'crates/*/src/*.rs' 'crates/*/src/**/*.rs' \
             'bins/*/src/*.rs' 'bins/*/src/**/*.rs' 'tools/*/src/*.rs' 'tools/*/src/**/*.rs' | sort -u); do
  [[ -f "$f" ]] || continue
  case "$f" in */tests.rs|*/test_support.rs|*/test_support/*) continue ;; esac
  n=$(grep -cE '^pub(\((crate|super)\))? (struct|enum|trait) ' "$f")
  if [[ "$f" != *_dto.rs && $n -gt 1 && ! "$f" =~ $allow ]]; then echo "multi-type($n) $f"; bad=1; fi
  if grep -qE '^\s*(pub(\(crate\))? )?mod tests \{' "$f"; then echo "inline-tests $f"; bad=1; fi
done
exit $bad
