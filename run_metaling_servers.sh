#!/usr/bin/env bash
set -euo pipefail
set +m

# Project Metaling: Server Runner and Runtime Verification Script.
# Usage:
#   ./run_metaling_servers.sh          Build, start all three servers and proxy, stream logs until interrupted.
#   ./run_metaling_servers.sh test     Build, start servers and proxy, verify endpoints, and cleanly exit.
#   ./run_metaling_servers.sh --help   Display usage information.

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
# Committed NPC spawn lists (GPL-3.0 data, see crates/ro-db/data/npc/NOTICE.md).
export NPC_DATA_DIR="${NPC_DATA_DIR:-$WORKSPACE_ROOT/crates/ro-db/data/npc}"

LOGIN_PORT="${LOGIN_PORT:-6910}"
CHAR_PORT="${CHAR_PORT:-6131}"
MAP_PORT="${MAP_PORT:-5131}"

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
            echo "  test            Start servers and proxy, verify endpoints, and exit."
            echo ""
            echo "Options:"
            echo "  --no-build      Skip compiling the server and proxy binaries."
            echo "  --release       Use release binaries in target/release/ directory."
            echo "  -h, --help      Show this help message."
            echo ""
            echo "Environment variables:"
            echo "  RUST_LOG        Log level filter (defaults to 'info')."
            echo "  LOGIN_PORT      Login server bind port (defaults to 6910)."
            echo "  CHAR_PORT       Char server bind port (defaults to 6131)."
            echo "  MAP_PORT        Map server bind port (defaults to 5131)."
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
PROXY_BIN="$BIN_DIR/proxy"

LOGIN_PID=""
CHAR_PID=""
MAP_PID=""
PROXY_PID=""

cleanup() {
    trap - INT TERM EXIT
    echo ""
    echo "[*] Stopping servers and proxy..."

    for pid in $LOGIN_PID $CHAR_PID $MAP_PID $PROXY_PID; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill "$pid" 2>/dev/null || true
        fi
    done

    for _ in {1..30}; do
        local still_running=0
        for pid in $LOGIN_PID $CHAR_PID $MAP_PID $PROXY_PID; do
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

    for pid in $LOGIN_PID $CHAR_PID $MAP_PID $PROXY_PID; do
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            kill -9 "$pid" 2>/dev/null || true
        fi
    done

    echo "[*] All servers and proxy stopped."
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
check_port 6900 "proxy login listener TCP"
check_port 6121 "proxy char listener TCP"
check_port 5121 "proxy map listener TCP"
check_port "$LOGIN_PORT" "login server bind TCP"
check_port "$CHAR_PORT" "char server bind TCP"
check_port "$MAP_PORT" "map server bind TCP"
check_port 6901 "login server HTTP"
check_port 6122 "char server HTTP"
check_port 5122 "map server HTTP"

if [[ "$BUILD_BINARIES" -eq 1 ]]; then
    echo "[*] Building server binaries and proxy ($BUILD_PROFILE profile)..."
    if [[ -n "$CARGO_PROFILE_FLAG" ]]; then
        cargo build --bins "$CARGO_PROFILE_FLAG"
        cargo build -p ro-proxy "$CARGO_PROFILE_FLAG"
    else
        cargo build --bins
        cargo build -p ro-proxy
    fi
fi

for bin in "$LOGIN_BIN" "$CHAR_BIN" "$MAP_BIN" "$PROXY_BIN"; do
    if [[ ! -x "$bin" ]]; then
        echo "Error: Binary not found or not executable: $bin" >&2
        exit 1
    fi
done

mkdir -p "$LOG_DIR"
LOGIN_LOG="$LOG_DIR/login-server.log"
CHAR_LOG="$LOG_DIR/char-server.log"
MAP_LOG="$LOG_DIR/map-server.log"
PROXY_LOG="$LOG_DIR/proxy.log"
rm -f "$LOGIN_LOG" "$CHAR_LOG" "$MAP_LOG" "$PROXY_LOG"

if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]]; then
    PREFIX_LOGIN="$(printf '\033[0;36m[login]\033[0m')"
    PREFIX_CHAR="$(printf '\033[0;32m[char]\033[0m')"
    PREFIX_MAP="$(printf '\033[0;35m[map]\033[0m')"
    PREFIX_PROXY="$(printf '\033[0;33m[proxy]\033[0m')"
else
    PREFIX_LOGIN="[login]"
    PREFIX_CHAR="[char]"
    PREFIX_MAP="[map]"
    PREFIX_PROXY="[proxy]"
fi

echo "[*] Starting Metaling servers..."

LOGIN_PORT="$LOGIN_PORT" "$LOGIN_BIN" > >(tee "$LOGIN_LOG" | awk -v p="$PREFIX_LOGIN" '{print p " " $0; fflush()}') 2>&1 &
LOGIN_PID=$!
disown "$LOGIN_PID" 2>/dev/null || true

CHAR_PORT="$CHAR_PORT" "$CHAR_BIN" > >(tee "$CHAR_LOG" | awk -v p="$PREFIX_CHAR" '{print p " " $0; fflush()}') 2>&1 &
CHAR_PID=$!
disown "$CHAR_PID" 2>/dev/null || true

MAP_PORT="$MAP_PORT" "$MAP_BIN" > >(tee "$MAP_LOG" | awk -v p="$PREFIX_MAP" '{print p " " $0; fflush()}') 2>&1 &
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

        if [[ "$login_ready" -eq 0 ]]; then
            if curl -s -f http://127.0.0.1:6901/api/v1/health >/dev/null 2>&1 && nc -z 127.0.0.1 "$LOGIN_PORT" 2>/dev/null; then
                login_ready=1
            fi
        fi

        if [[ "$char_ready" -eq 0 ]]; then
            if curl -s -f http://127.0.0.1:6122/api/v1/health >/dev/null 2>&1 && nc -z 127.0.0.1 "$CHAR_PORT" 2>/dev/null; then
                char_ready=1
            fi
        fi

        if [[ "$map_ready" -eq 0 ]]; then
            if curl -s -f http://127.0.0.1:5122/api/v1/health >/dev/null 2>&1 && nc -z 127.0.0.1 "$MAP_PORT" 2>/dev/null; then
                map_ready=1
            fi
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

echo "[*] Waiting for server health endpoints and TCP bind ports..."
if ! verify_servers_healthy; then
    exit 1
fi

echo "[*] All three servers are healthy and accepting connections."

echo "[*] Starting proxy..."
"$PROXY_BIN" \
    --target-ip 127.0.0.1 \
    --login "127.0.0.1:$LOGIN_PORT" \
    --char "127.0.0.1:$CHAR_PORT" \
    --map "127.0.0.1:$MAP_PORT" \
    --listen-login-port 6900 \
    --listen-char-port 6121 \
    --listen-map-port 5121 \
    --log-file "$LOG_DIR/packet_sequence.log" \
    > >(tee "$PROXY_LOG" | awk -v p="$PREFIX_PROXY" '{print p " " $0; fflush()}') 2>&1 &
PROXY_PID=$!
disown "$PROXY_PID" 2>/dev/null || true

echo "[*] Launched proxy (PID: $PROXY_PID)"

verify_proxy_healthy() {
    local start_time
    start_time=$(date +%s)
    local proxy_timeout=15

    while true; do
        if ! kill -0 "$PROXY_PID" 2>/dev/null; then
            echo "Error: proxy (PID: $PROXY_PID) exited unexpectedly." >&2
            echo "Log contents of $PROXY_LOG:" >&2
            cat "$PROXY_LOG" >&2 || true
            return 1
        fi

        if nc -z 127.0.0.1 6900 2>/dev/null && nc -z 127.0.0.1 6121 2>/dev/null && nc -z 127.0.0.1 5121 2>/dev/null; then
            return 0
        fi

        local current_time
        current_time=$(date +%s)
        if (( current_time - start_time >= proxy_timeout )); then
            echo "Error: Timeout reached waiting for proxy ports to become reachable." >&2
            return 1
        fi

        sleep 0.2
    done
}

echo "[*] Waiting for proxy listener ports..."
if ! verify_proxy_healthy; then
    exit 1
fi

echo "[*] Proxy is active and accepting connections."

if [[ "$MODE" == "test" ]]; then
    echo ""
    echo "=== Runtime Health Status ==="
    echo "Login Server (HTTP 6901): $(curl -s http://127.0.0.1:6901/api/v1/health)"
    echo "Char Server  (HTTP 6122): $(curl -s http://127.0.0.1:6122/api/v1/health)"
    echo "Map Server   (HTTP 5122): $(curl -s http://127.0.0.1:5122/api/v1/health)"
    echo ""
    echo "=== Metaling Direct TCP Ports ==="
    nc -z 127.0.0.1 "$LOGIN_PORT" && echo "Login server TCP port $LOGIN_PORT reachable."
    nc -z 127.0.0.1 "$CHAR_PORT" && echo "Char server TCP port $CHAR_PORT reachable."
    nc -z 127.0.0.1 "$MAP_PORT" && echo "Map server TCP port $MAP_PORT reachable."
    echo ""
    echo "=== Proxy Client TCP Ports ==="
    nc -z 127.0.0.1 6900 && echo "Proxy login client TCP port 6900 reachable."
    nc -z 127.0.0.1 6121 && echo "Proxy char client TCP port 6121 reachable."
    nc -z 127.0.0.1 5121 && echo "Proxy map client TCP port 5121 reachable."
    echo ""
    echo "[*] Test verification completed successfully."
    exit 0
fi

echo "[*] Streaming server and proxy logs. Press Ctrl+C to terminate."
while kill -0 "$LOGIN_PID" 2>/dev/null && kill -0 "$CHAR_PID" 2>/dev/null && kill -0 "$MAP_PID" 2>/dev/null && kill -0 "$PROXY_PID" 2>/dev/null; do
    sleep 1
done
echo "A server or proxy process terminated unexpectedly. Initiating shutdown." >&2
exit 1
