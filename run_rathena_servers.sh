#!/usr/bin/env bash
set -euo pipefail
set +m

# rAthena and Proxy Runner Script.
# Usage:
#   ./run_rathena_servers.sh          Start rAthena containers and proxy, stream logs until interrupted.
#   ./run_rathena_servers.sh test     Start rAthena and proxy, verify endpoints, and cleanly exit.
#   ./run_rathena_servers.sh --help   Display usage information.

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_ROOT="$SCRIPT_DIR"
cd "$WORKSPACE_ROOT"

BUILD_PROXY=1
BUILD_PROFILE="debug"
CARGO_PROFILE_FLAG=""
MODE="run"
TIMEOUT_SECONDS=30
LOG_DIR="$WORKSPACE_ROOT/target/logs"
export RUST_LOG="${RUST_LOG:-info}"

RATHENA_LOGIN_PORT=6910
RATHENA_CHAR_PORT=6131
RATHENA_MAP_PORT=5131
RATHENA_DB_PORT=3306

PROXY_LOGIN_PORT=6900
PROXY_CHAR_PORT=6121
PROXY_MAP_PORT=5121

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
            BUILD_PROXY=0
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
            echo "  run             Start rAthena and proxy, stream logs until interrupted (default)."
            echo "  test            Start rAthena and proxy, verify endpoints, and exit."
            echo ""
            echo "Options:"
            echo "  --no-build      Skip compiling the proxy binary."
            echo "  --release       Use release binary in target/release/ directory."
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

DOCKER_COMPOSE_FILE="$RATHENA_PATH/tools/docker/docker-compose.yml"
if docker compose version >/dev/null 2>&1; then
    DOCKER_COMPOSE_CMD=(docker compose -f "$DOCKER_COMPOSE_FILE")
elif command -v docker-compose >/dev/null 2>&1; then
    DOCKER_COMPOSE_CMD=(docker-compose -f "$DOCKER_COMPOSE_FILE")
else
    echo "Error: Neither 'docker compose' nor 'docker-compose' is available." >&2
    exit 1
fi

BIN_DIR="$WORKSPACE_ROOT/target/$BUILD_PROFILE"
PROXY_BIN="$BIN_DIR/proxy"
PROXY_PID=""

cleanup() {
    trap - INT TERM EXIT
    echo ""
    echo "[*] Stopping rAthena proxy and containers..."

    if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
        kill "$PROXY_PID" 2>/dev/null || true
    fi

    for _ in {1..30}; do
        if [[ -z "$PROXY_PID" ]] || ! kill -0 "$PROXY_PID" 2>/dev/null; then
            break
        fi
        sleep 0.1
    done

    if [[ -n "$PROXY_PID" ]] && kill -0 "$PROXY_PID" 2>/dev/null; then
        kill -9 "$PROXY_PID" 2>/dev/null || true
    fi

    if [[ ${#DOCKER_COMPOSE_CMD[@]} -gt 0 ]]; then
        "${DOCKER_COMPOSE_CMD[@]}" down 2>/dev/null || true
    fi

    echo "[*] rAthena stack and proxy stopped."
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
check_port "$RATHENA_DB_PORT" "MySQL database"
check_port "$RATHENA_LOGIN_PORT" "rAthena login host port"
check_port "$RATHENA_CHAR_PORT" "rAthena char host port"
check_port "$RATHENA_MAP_PORT" "rAthena map host port"
check_port "$PROXY_LOGIN_PORT" "proxy login listener TCP"
check_port "$PROXY_CHAR_PORT" "proxy char listener TCP"
check_port "$PROXY_MAP_PORT" "proxy map listener TCP"

if [[ "$BUILD_PROXY" -eq 1 ]]; then
    echo "[*] Building proxy ($BUILD_PROFILE profile)..."
    if [[ -n "$CARGO_PROFILE_FLAG" ]]; then
        cargo build -p ro-proxy "$CARGO_PROFILE_FLAG"
    else
        cargo build -p ro-proxy
    fi
fi

if [[ ! -x "$PROXY_BIN" ]]; then
    echo "Error: Binary not found or not executable: $PROXY_BIN" >&2
    exit 1
fi

echo "[*] Starting rAthena containers via docker compose..."
"${DOCKER_COMPOSE_CMD[@]}" up -d

verify_rathena_ready() {
    local start_time
    start_time=$(date +%s)
    local login_ready=0
    local char_ready=0
    local map_ready=0

    while true; do
        if [[ "$login_ready" -eq 0 ]] && nc -z 127.0.0.1 "$RATHENA_LOGIN_PORT" 2>/dev/null; then
            login_ready=1
        fi
        if [[ "$char_ready" -eq 0 ]] && nc -z 127.0.0.1 "$RATHENA_CHAR_PORT" 2>/dev/null; then
            char_ready=1
        fi
        if [[ "$map_ready" -eq 0 ]] && nc -z 127.0.0.1 "$RATHENA_MAP_PORT" 2>/dev/null; then
            map_ready=1
        fi

        if [[ "$login_ready" -eq 1 ]] && [[ "$char_ready" -eq 1 ]] && [[ "$map_ready" -eq 1 ]]; then
            return 0
        fi

        local current_time
        current_time=$(date +%s)
        if (( current_time - start_time >= TIMEOUT_SECONDS )); then
            echo "Error: Timeout reached waiting for rAthena ports to become ready." >&2
            echo "Login ready: $login_ready, Char ready: $char_ready, Map ready: $map_ready" >&2
            return 1
        fi

        sleep 0.5
    done
}

echo "[*] Waiting for rAthena server ports ($RATHENA_LOGIN_PORT, $RATHENA_CHAR_PORT, $RATHENA_MAP_PORT)..."
if ! verify_rathena_ready; then
    exit 1
fi

echo "[*] All rAthena servers are listening on host ports."

mkdir -p "$LOG_DIR"
RATHENA_PROXY_LOG="$LOG_DIR/rathena-proxy.log"
rm -f "$RATHENA_PROXY_LOG"

if [[ -t 1 ]] && [[ -z "${NO_COLOR:-}" ]]; then
    PREFIX_PROXY="$(printf '\033[0;33m[proxy]\033[0m')"
else
    PREFIX_PROXY="[proxy]"
fi

echo "[*] Starting proxy..."
"$PROXY_BIN" \
    --target-ip 127.0.0.1 \
    --login "127.0.0.1:$RATHENA_LOGIN_PORT" \
    --char "127.0.0.1:$RATHENA_CHAR_PORT" \
    --map "127.0.0.1:$RATHENA_MAP_PORT" \
    --listen-login-port "$PROXY_LOGIN_PORT" \
    --listen-char-port "$PROXY_CHAR_PORT" \
    --listen-map-port "$PROXY_MAP_PORT" \
    --log-file "$LOG_DIR/rathena_packet_sequence.log" \
    > >(tee "$RATHENA_PROXY_LOG" | awk -v p="$PREFIX_PROXY" '{print p " " $0; fflush()}') 2>&1 &
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
            echo "Log contents of $RATHENA_PROXY_LOG:" >&2
            cat "$RATHENA_PROXY_LOG" >&2 || true
            return 1
        fi

        if nc -z 127.0.0.1 "$PROXY_LOGIN_PORT" 2>/dev/null && nc -z 127.0.0.1 "$PROXY_CHAR_PORT" 2>/dev/null && nc -z 127.0.0.1 "$PROXY_MAP_PORT" 2>/dev/null; then
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
    echo "=== rAthena Exposed Host Ports ==="
    nc -z 127.0.0.1 "$RATHENA_LOGIN_PORT" && echo "rAthena login host port $RATHENA_LOGIN_PORT reachable."
    nc -z 127.0.0.1 "$RATHENA_CHAR_PORT" && echo "rAthena char host port $RATHENA_CHAR_PORT reachable."
    nc -z 127.0.0.1 "$RATHENA_MAP_PORT" && echo "rAthena map host port $RATHENA_MAP_PORT reachable."
    echo ""
    echo "=== Proxy Client TCP Ports ==="
    nc -z 127.0.0.1 "$PROXY_LOGIN_PORT" && echo "Proxy login client TCP port $PROXY_LOGIN_PORT reachable."
    nc -z 127.0.0.1 "$PROXY_CHAR_PORT" && echo "Proxy char client TCP port $PROXY_CHAR_PORT reachable."
    nc -z 127.0.0.1 "$PROXY_MAP_PORT" && echo "Proxy map client TCP port $PROXY_MAP_PORT reachable."
    echo ""
    echo "[*] Test verification completed successfully."
    exit 0
fi

echo "[*] Streaming proxy logs. Press Ctrl+C to terminate."
while kill -0 "$PROXY_PID" 2>/dev/null; do
    sleep 1
done
echo "Proxy process terminated unexpectedly. Initiating shutdown." >&2
exit 1
