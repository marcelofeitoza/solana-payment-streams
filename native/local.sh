#!/usr/bin/env bash
# Build, start, stop, reset, or demo the ordinary-Solana implementation.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RPC_PORT="${RPC_PORT:-9910}"
WS_PORT="${WS_PORT:-9911}"
STUDIO_PORT="${STUDIO_PORT:-19910}"
RUNTIME="$ROOT/.local/runtime"
PID_FILE="$RUNTIME/surfpool.pid"
IDENTITY_FILE="$RUNTIME/surfpool.identity"
LOG_FILE="$RUNTIME/surfpool.log"
MANIFEST="$ROOT/txtx.yml"
ARTIFACT="$ROOT/target/deploy/native_usdc_stream_program.so"

build() {
    cargo build-sbf --manifest-path "$ROOT/program/Cargo.toml" --sbf-out-dir "$ROOT/target/deploy"
    test -s "$ARTIFACT"
    shasum -a 256 "$ARTIFACT"
}

start() {
    build
    mkdir -p "$RUNTIME"
    for port in "$RPC_PORT" "$WS_PORT" "$STUDIO_PORT"; do
        if nc -z 127.0.0.1 "$port" >/dev/null 2>&1; then
            echo "port $port is busy; refusing to attach" >&2
            return 1
        fi
    done
    if [[ -f "$PID_FILE" ]] && kill -0 "$(tr -cd '0-9' < "$PID_FILE")" 2>/dev/null; then
        echo "this project already owns a live Surfpool process" >&2
        return 1
    fi
    (
        cd "$ROOT"
        exec surfpool start --offline --port "$RPC_PORT" --ws-port "$WS_PORT" \
            --studio-port "$STUDIO_PORT" --slot-time 5 --db :memory: --no-tui --no-studio \
            --disable-instruction-profiling --manifest-file-path "$MANIFEST" \
            --runbook deployment --yes --log-level warn
    ) >"$LOG_FILE" 2>&1 &
    local pid=$!
    echo "$pid" > "$PID_FILE"
    ps -p "$pid" -o lstart= | sed 's/^[[:space:]]*//' > "$IDENTITY_FILE"
    for _ in $(seq 1 400); do
        kill -0 "$pid" 2>/dev/null || { tail -n 80 "$LOG_FILE" >&2; return 1; }
        curl -sf --max-time 1 -H 'content-type: application/json' \
            -d '{"jsonrpc":"2.0","id":1,"method":"getVersion"}' \
            "http://127.0.0.1:$RPC_PORT" | grep -q result && {
                echo "surfpool_rpc=http://127.0.0.1:$RPC_PORT pid=$pid"
                return
            }
        sleep 0.025
    done
    echo "Surfpool did not become ready" >&2
    return 1
}

stop() {
    [[ -f "$PID_FILE" ]] || return 0
    local pid command started expected
    pid="$(tr -cd '0-9' < "$PID_FILE")"
    kill -0 "$pid" 2>/dev/null || { mv "$PID_FILE" "$PID_FILE.stopped"; return 0; }
    command="$(ps -p "$pid" -o command=)"
    started="$(ps -p "$pid" -o lstart= | sed 's/^[[:space:]]*//')"
    expected="$(cat "$IDENTITY_FILE" 2>/dev/null || true)"
    if [[ "$started" != "$expected" || "$command" != *"surfpool start"* || "$command" != *"--port $RPC_PORT"* ]]; then
        echo "PID $pid is not this project's Surfpool process; refusing to stop it" >&2
        return 1
    fi
    kill -TERM "$pid"
    for _ in $(seq 1 400); do kill -0 "$pid" 2>/dev/null || break; sleep 0.025; done
    kill -0 "$pid" 2>/dev/null && { echo "Surfpool did not stop" >&2; return 1; }
    mv "$PID_FILE" "$PID_FILE.stopped"
}

case "${1:-demo}" in
    build) build ;;
    start) start ;;
    stop) stop ;;
    reset) stop; [[ ! -d "$RUNTIME" ]] || mv "$RUNTIME" "$ROOT/.local/runtime.$(date +%s)" ;;
    demo) start; trap stop EXIT INT TERM; cd "$ROOT"; npm install; npm run demo ;;
    *) echo "usage: $0 {build|start|stop|reset|demo}" >&2; exit 2 ;;
esac
