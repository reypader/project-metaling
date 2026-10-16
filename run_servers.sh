#!/usr/bin/env bash
set -euo pipefail
set +m

# Project Metaling: Server Runner and Runtime Verification Script.
# Usage:
#   ./run_servers.sh          # Build, start all three servers, stream prefixed logs, run until interrupted
#   ./run_servers.sh test     # Build, start all three servers, verify endpoints, and cleanly exit
#   ./run_servers.sh --help   # Display usage information

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$SCRIPT_DIR"
cd "$WORKSPACE_ROOT"

BUILD_BINARIES=1
BUILD_PROFILE="debug"
CARGO_PROFILE_FLAG=""
MODE="run"
TIMEOUT_SECONDS=15
LOG_DIR="$WORKSPACE_ROOT/target/logs"
export RUST_LOG="${RUST_LOG:-info}"

while [[ $# -gt 0 ]]; do
    case "$1" in
        run)
            MODE="run"
            shift
            ;;
        test|--test)
            MODE="test"
            shift
            ;;
        --no-build)
            BUILD_BINARIES=0
            shift
            ;;
        --release)
            BUILD_PROFILE="release"
            CARGO_PROFILE_FLAG="--release"
            shift
            ;;
        -h|--help)
            echo "Usage: $0 [MODE] [OPTIONS]"
            echo ""
            echo "Modes:"
            echo "  run             Start servers and stream logs until interrupted (default)."
            echo "  test            Start servers, verify health endpoints, and exit."
            echo ""
            echo "Options:"
            echo "  --no-build      Skip compiling the server binaries."
            echo "  --release       Use release binaries in target/release/ instead of target/debug/."
            echo "  -h, --help      Show this help message."
            echo ""
            echo "Environment variables:"
            echo "  RUST_LOG        Log level filter (defaults to 'info')."
            exit 0
            ;;
        *)
            echo "Unknown argument: $1" >&2
            echo "Run '$0 --help' for usage." >&2
            exit 1
            ;;
    esac
done

BIN_DIR="$WORKSPACE_ROOT/target/$BUILD_PROFILE"
LOGIN_BIN="$BIN_DIR/login-server"
CHAR_BIN="$BIN_DIR/char-server"
MAP_BIN="$BIN_DIR/map-server"

LOGIN_PID=""
CHAR_PID=""
MAP_PID=""

cleanup() {
    trap - INT TERM EXIT
    echo ""
    echo "[*] Stopping servers..."

    for pid in $LOGIN_PID $CHAR_PID $MAP_PID; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done

    for _ in {1..30}; do
        local still_running=0
        for pid in $LOGIN_PID $CHAR_PID $MAP_PID; do
            if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
                still_running=1
                break
            fi
        done
        if [[ "$still_running" -eq 0 ]]; then
            break
        fi
        sleep 0.1
    done

    for pid in $LOGIN_PID $CHAR_PID $MAP_PID; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill -9 "$pid" 2>/dev/null || true
        fi
    done

    echo "[*] All servers stopped."
}

trap cleanup INT TERM EXIT

check_port() {
    local port=$1
    local name=$2
    if nc -z 127.0.0.1 "$port" 2>/dev/null; then
        echo "Error: Port $port ($name) is already in use." >&2
        exit 1
    fi
}

echo "[*] Checking port availability..."
check_port 6900 "login client TCP"
check_port 6901 "login HTTP"
check_port 6121 "char client TCP"
check_port 6122 "char HTTP"
check_port 5121 "map client TCP"
check_port 5122 "map HTTP"

if [[ "$BUILD_BINARIES" -eq 1 ]]; then
    echo "[*] Building server binaries ($BUILD_PROFILE profile)..."
    if [[ -n "$CARGO_PROFILE_FLAG" ]]; then
        cargo build --bins "$CARGO_PROFILE_FLAG"
    else
        cargo build --bins
    fi
fi

for bin in "$LOGIN_BIN" "$CHAR_BIN" "$MAP_BIN"; do
    if [[ ! -x "$bin" ]]; then
        echo "Error: Binary not found or not executable: $bin" >&2
        exit 1
    fi
done

mkdir -p "$LOG_DIR"
LOGIN_LOG="$LOG_DIR/login-server.log"
CHAR_LOG="$LOG_DIR/char-server.log"
MAP_LOG="$LOG_DIR/map-server.log"
rm -f "$LOGIN_LOG" "$CHAR_LOG" "$MAP_LOG"

if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]]; then
    PREFIX_LOGIN="$(printf '\033[0;36m[login]\033[0m')"
    PREFIX_CHAR="$(printf '\033[0;32m[char]\033[0m')"
    PREFIX_MAP="$(printf '\033[0;35m[map]\033[0m')"
else
    PREFIX_LOGIN="[login]"
    PREFIX_CHAR="[char]"
    PREFIX_MAP="[map]"
fi

echo "[*] Starting servers..."

"$LOGIN_BIN" > >(tee "$LOGIN_LOG" | awk -v p="$PREFIX_LOGIN" '{print p " " $0; fflush()}') 2>&1 &
LOGIN_PID=$!
disown "$LOGIN_PID" 2>/dev/null || true

"$CHAR_BIN" > >(tee "$CHAR_LOG" | awk -v p="$PREFIX_CHAR" '{print p " " $0; fflush()}') 2>&1 &
CHAR_PID=$!
disown "$CHAR_PID" 2>/dev/null || true

"$MAP_BIN" > >(tee "$MAP_LOG" | awk -v p="$PREFIX_MAP" '{print p " " $0; fflush()}') 2>&1 &
MAP_PID=$!
disown "$MAP_PID" 2>/dev/null || true

echo "[*] Launched login-server (PID: $LOGIN_PID), char-server (PID: $CHAR_PID), map-server (PID: $MAP_PID)"

verify_servers_healthy() {
    local start_time
    start_time=$(date +%s)
    local login_ready=0
    local char_ready=0
    local map_ready=0

    while true; do
        for pid_var in "LOGIN_PID:$LOGIN_PID:login-server:$LOGIN_LOG" "CHAR_PID:$CHAR_PID:char-server:$CHAR_LOG" "MAP_PID:$MAP_PID:map-server:$MAP_LOG"; do
            IFS=":" read -r _ pid name log_file <<< "$pid_var"
            if ! kill -0 "$pid" 2>/dev/null; then
                echo "Error: $name (PID: $pid) exited unexpectedly." >&2
                echo "Log contents of $log_file:" >&2
                cat "$log_file" >&2 || true
                return 1
            fi
        done

        if [[ "$login_ready" -eq 0 ]] && curl -s -f http://127.0.0.1:6901/api/v1/health >/dev/null 2>&1; then
            login_ready=1
        fi

        if [[ "$char_ready" -eq 0 ]] && curl -s -f http://127.0.0.1:6122/api/v1/health >/dev/null 2>&1; then
            char_ready=1
        fi

        if [[ "$map_ready" -eq 0 ]] && curl -s -f http://127.0.0.1:5122/api/v1/health >/dev/null 2>&1; then
            map_ready=1
        fi

        if [[ "$login_ready" -eq 1 ]] && [[ "$char_ready" -eq 1 ]] && [[ "$map_ready" -eq 1 ]]; then
            return 0
        fi

        local current_time
        current_time=$(date +%s)
        if (( current_time - start_time >= TIMEOUT_SECONDS )); then
            echo "Error: Timeout reached waiting for servers to become ready." >&2
            echo "Login ready: $login_ready, Char ready: $char_ready, Map ready: $map_ready" >&2
            return 1
        fi

        sleep 0.2
    done
}

echo "[*] Waiting for server health endpoints..."
if ! verify_servers_healthy; then
    exit 1
fi

echo "[*] All three servers are healthy and accepting connections."

if [[ "$MODE" == "test" ]]; then
    echo ""
    echo "=== Runtime Health Status ==="
    echo "Login Server (HTTP 6901): $(curl -s http://127.0.0.1:6901/api/v1/health)"
    echo "Char Server  (HTTP 6122): $(curl -s http://127.0.0.1:6122/api/v1/health)"
    echo "Map Server   (HTTP 5122): $(curl -s http://127.0.0.1:5122/api/v1/health)"
    echo ""
    echo "=== TCP Port Verification ==="
    nc -z 127.0.0.1 6900 && echo "Login client TCP port 6900 reachable."
    nc -z 127.0.0.1 6121 && echo "Char client TCP port 6121 reachable."
    nc -z 127.0.0.1 5121 && echo "Map client TCP port 5121 reachable."
    echo ""
    echo "[*] Test verification completed successfully."
    exit 0
fi

echo "[*] Streaming server logs. Press Ctrl+C to terminate."
while kill -0 "$LOGIN_PID" 2>/dev/null && kill -0 "$CHAR_PID" 2>/dev/null && kill -0 "$MAP_PID" 2>/dev/null; do
    sleep 1
done
echo "A server process terminated unexpectedly. Initiating shutdown." >&2
exit 1
