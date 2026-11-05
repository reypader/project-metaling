#!/usr/bin/env bash
# REF-02: print "<lib.rs path> <exported name>" for every root re-export, sorted.
# Usage: specs/file-structure/public_api.sh | diff specs/file-structure/public_api.txt -
# Glob re-exports (`pub use x::*`) expand to the `pub` items declared under the crate's src/.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"
for lib in $(git ls-files '*/src/lib.rs'); do
  src=$(dirname "$lib")
  awk '/^pub use/{p=1} p{printf "%s ", $0} p&&/;/{print ""; p=0}' "$lib" | while read -r stmt; do
    if [[ "$stmt" == *'::*;'* ]]; then
      grep -rhoE '^pub (struct|enum|trait|type|const|fn|static) [A-Za-z0-9_]+' "$src" | awk '{print $3}'
    else
      body=${stmt#pub use }; body=${body%;}
      if [[ "$body" == *'{'* ]]; then body=${body#*\{}; body=${body%\}*}; fi
      tr ',' '\n' <<<"$body" | sed -E 's/.* as //; s/.*:://; s/[[:space:]]//g' | grep -v '^$'
    fi
  done | sed "s|^|$lib |"
  # pub items defined directly in lib.rs (e.g. the import tools)
  grep -oE '^pub (struct|enum|trait|type|const|fn|static) [A-Za-z0-9_]+' "$lib" | awk -v l="$lib" '{print l, $3}' || true
done | sort -u
