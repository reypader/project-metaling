#!/usr/bin/env bash
# Compact verification: fmt -> clippy (also covers `cargo check`) -> tests.
# Prints only a summary and the actionable lines; full logs go to target/verify/<step>.log.
#
# Usage:
#   ./verify.sh                 whole workspace
#   ./verify.sh -p ro-packets   scope clippy + tests to one package (repeat -p for more)
#   ./verify.sh --no-test       skip tests
#   ./verify.sh --smoke         also run ./run_metaling_servers.sh test
# Exit code is non-zero if any step fails.
set -uo pipefail
cd "$(dirname "$0")"

LOG_DIR=target/verify
MAX_LINES=${VERIFY_MAX_LINES:-40}   # cap per section so a bad run can't flood the context
mkdir -p "$LOG_DIR"

scope=(--workspace)
fmt_scope=(--all)
run_tests=1
run_smoke=0
pkgs=()
while [[ $# -gt 0 ]]; do
    case "$1" in
        -p|--package) pkgs+=(-p "$2"); shift 2 ;;
        --no-test) run_tests=0; shift ;;
        --smoke) run_smoke=1; shift ;;
        -h|--help) sed -n '2,10p' "$0"; exit 0 ;;
        *) echo "unknown arg: $1" >&2; exit 2 ;;
    esac
done
if [[ ${#pkgs[@]} -gt 0 ]]; then scope=("${pkgs[@]}"); fmt_scope=("${pkgs[@]}"); fi

status=0
cap() { head -n "$MAX_LINES"; }

# --- fmt: list files only, not diffs ---
if cargo fmt "${fmt_scope[@]}" --check -- -l >"$LOG_DIR/fmt.log" 2>&1; then
    echo "fmt: ok"
else
    n=$(grep -c . "$LOG_DIR/fmt.log")
    echo "fmt: FAIL ($n files; fix with: cargo fmt ${fmt_scope[*]})"
    sed "s|^$PWD/|  |" "$LOG_DIR/fmt.log" | cap
    status=1
fi

# --- clippy: one line per diagnostic, deduped across lib/test targets ---
cargo clippy "${scope[@]}" --all-targets --message-format=short >"$LOG_DIR/clippy.log" 2>&1
clippy_rc=$?
diags=$(grep -E '^[^ ]+:[0-9]+:[0-9]+: (warning|error)|^error(\[|:)' "$LOG_DIR/clippy.log" \
    | grep -v -E '^error: could not compile|generated [0-9]+ warnings?' | sort -u)
if [[ $clippy_rc -eq 0 && -z "$diags" ]]; then
    echo "clippy: ok"
else
    echo "clippy: FAIL ($(grep -c . <<<"$diags") diagnostics; full: $LOG_DIR/clippy.log)"
    cap <<<"$diags"
    status=1
    if [[ $clippy_rc -ne 0 ]]; then
        echo "tests: skipped (build failed)"; run_tests=0
    fi
fi

# --- tests: aggregate counts, show only failures + panic messages ---
if [[ $run_tests -eq 1 ]]; then
    cargo test "${scope[@]}" -q --no-fail-fast >"$LOG_DIR/test.log" 2>&1
    test_rc=$?
    awk '/^test result:/ { p += $4; f += $6; i += $8 }
         END { printf "tests: %s (%d passed, %d failed, %d ignored)\n", (f ? "FAIL" : "ok"), p, f, i }' \
        "$LOG_DIR/test.log"
    if [[ $test_rc -ne 0 ]]; then
        # panic location + the message line after it; rerun hints name the failing test binary
        awk '/panicked at/ { print "  " $0; getline; print "    " $0; next }
             /^error: test failed/ { print "  " $0 }
             /^error(\[|:)/ && !/test failed/ { print "  " $0 }' "$LOG_DIR/test.log" | cap
        echo "  full: $LOG_DIR/test.log"
        status=1
    fi
fi

# --- optional end-to-end smoke test ---
if [[ $run_smoke -eq 1 ]]; then
    if ./run_metaling_servers.sh test >"$LOG_DIR/smoke.log" 2>&1; then
        echo "smoke: ok"
    else
        echo "smoke: FAIL (full: $LOG_DIR/smoke.log)"
        grep -iE 'error|fail|panic' "$LOG_DIR/smoke.log" | tail -n 15
        status=1
    fi
fi

exit $status
