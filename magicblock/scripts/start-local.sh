#!/usr/bin/env bash
# Start Surfpool, deploy through IaC, initialize DLP fees, then start the local ER.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUNTIME_DIR="${POC_RUNTIME_DIR:-$PROJECT_ROOT/.local/runtime}"
VALIDATOR_BIN="$PROJECT_ROOT/.local-tools/ephemeral-validator"
SURFPOOL_PID_FILE="$RUNTIME_DIR/surfpool.pid"
ER_PID_FILE="$RUNTIME_DIR/ephemeral-validator.pid"

if [[ ! -x "$VALIDATOR_BIN" \
    || ! -f "$PROJECT_ROOT/target/deploy/magicblock_usdc_stream.so" \
    || ! -f "$PROJECT_ROOT/target/deploy/spl_noop.so" \
    || ! -f "$PROJECT_ROOT/.local-tools/delegation-program/target/deploy/dlp.so" ]]; then
    echo "local artifacts are missing; run scripts/setup.sh first" >&2
    exit 1
fi
if ! command -v surfpool >/dev/null 2>&1; then
    echo "missing required command: surfpool" >&2
    exit 1
fi
if ! command -v npx >/dev/null 2>&1; then
    echo "missing required command: npx (install Node.js)" >&2
    exit 1
fi
if [[ ! -d "$PROJECT_ROOT/node_modules" ]]; then
    echo "TS client dependencies are missing; run 'npm install' in $PROJECT_ROOT first" >&2
    exit 1
fi

mkdir -p "$RUNTIME_DIR"

check_stale_pid() {
    local pid_file="$1"
    local label="$2"
    if [[ -f "$pid_file" ]]; then
        local existing_pid
        existing_pid="$(tr -cd '0-9' < "$pid_file")"
        if [[ -n "$existing_pid" ]] && kill -0 "$existing_pid" 2>/dev/null; then
            echo "$label is already running with pid $existing_pid" >&2
            exit 1
        fi
        rm -f "$pid_file"
    fi
}

check_stale_pid "$SURFPOOL_PID_FILE" "Surfpool"
check_stale_pid "$ER_PID_FILE" "ephemeral-validator"

rpc_ready() {
    local rpc_url="$1"
    local response
    response="$(curl --silent --max-time 1 \
        --header 'content-type: application/json' \
        --data '{"jsonrpc":"2.0","id":1,"method":"getVersion"}' \
        "$rpc_url" 2>/dev/null || true)"
    [[ "$response" == *'"result"'* ]]
}

for reserved_port in 9900 9901 17899 17900 19099 19100; do
    if nc -z 127.0.0.1 "$reserved_port" >/dev/null 2>&1; then
        echo "required local port $reserved_port is already in use" >&2
        exit 1
    fi
done

# `ephemeral-validator --reset` resets its ledger and task scheduler, but
# v0.13.19 intentionally preserves the committor recovery database. This PoC
# starts a brand-new Surfpool ledger each time, so recovered intents would
# target accounts from a different local chain and block the new queue.
rm -f -- \
    "$RUNTIME_DIR/er-storage/committor_service.sqlite" \
    "$RUNTIME_DIR/er-storage/committor_service.sqlite-shm" \
    "$RUNTIME_DIR/er-storage/committor_service.sqlite-wal"

cleanup_on_error() {
    "$SCRIPT_DIR/stop-local.sh" >/dev/null 2>&1 || true
}
trap cleanup_on_error ERR INT TERM

echo "Starting offline Surfpool at http://127.0.0.1:9900"
(
    cd "$PROJECT_ROOT"
    # The MagicBlock RPC client caches a blockhash for five seconds. A 50 ms
    # slot leaves each Surfpool blockhash valid for about 7.5 seconds (150
    # slots), while remaining fast enough for the 10,000 local settlements.
    # Shorter slots can expire an otherwise fresh cached blockhash mid-run.
    exec surfpool start \
        --offline \
        --port 9900 \
        --ws-port 9901 \
        --studio-port 19100 \
        --slot-time 50 \
        --db :memory: \
        --surfnet-id magicblock-usdc-crank-poc \
        --manifest-file-path "$PROJECT_ROOT/txtx.yml" \
        --runbook deployment \
        --yes \
        --no-tui \
        --no-studio \
        --log-level warn
) >"$RUNTIME_DIR/surfpool.log" 2>&1 &
surfpool_pid=$!
echo "$surfpool_pid" > "$SURFPOOL_PID_FILE"

wait_for_rpc() {
    local rpc_url="$1"
    local pid="$2"
    local log_file="$3"
    local label="$4"
    for _attempt in $(seq 1 300); do
        if ! kill -0 "$pid" 2>/dev/null; then
            echo "$label exited during startup" >&2
            tail -n 80 "$log_file" >&2 || true
            return 1
        fi
        if rpc_ready "$rpc_url"; then
            return 0
        fi
        sleep 0.1
    done
    echo "timed out waiting for $label at $rpc_url" >&2
    tail -n 80 "$log_file" >&2 || true
    return 1
}

wait_for_rpc "http://127.0.0.1:9900" "$surfpool_pid" "$RUNTIME_DIR/surfpool.log" "Surfpool"

echo "Assigning the official local validator as DLP authority through Surfpool IaC"
(
    cd "$PROJECT_ROOT"
    surfpool run dlp-authority \
        --manifest-file-path "$PROJECT_ROOT/txtx.yml" \
        --env localnet \
        --unsupervised \
        --force
)

echo "Initializing local DLP protocol and validator fee vaults with the TS client"
(
    cd "$PROJECT_ROOT"
    BASE_RPC=http://127.0.0.1:9900 \
    ER_RPC=http://127.0.0.1:17899 \
    STATE_DIR="$PROJECT_ROOT/.local/bootstrap" \
        npx tsx client/index.ts bootstrap-local-dlp
)

echo "Starting MagicBlock Ephemeral Rollup at http://127.0.0.1:17899"
(
    cd "$PROJECT_ROOT"
    exec "$VALIDATOR_BIN" "$PROJECT_ROOT/config/ephemeral-validator.toml" \
        --remotes http://127.0.0.1:9900 \
        --remotes ws://127.0.0.1:9901 \
        --storage "$RUNTIME_DIR/er-storage" \
        --listen 127.0.0.1:17899 \
        --lifecycle ephemeral \
        --no-tui \
        --reset
) >"$RUNTIME_DIR/ephemeral-validator.log" 2>&1 &
er_pid=$!
echo "$er_pid" > "$ER_PID_FILE"

wait_for_rpc "http://127.0.0.1:17899" "$er_pid" "$RUNTIME_DIR/ephemeral-validator.log" "ephemeral-validator"
trap - ERR INT TERM

echo "Local stack is ready"
echo "Surfpool RPC: http://127.0.0.1:9900 (pid $surfpool_pid)"
echo "Ephemeral RPC: http://127.0.0.1:17899 (pid $er_pid)"
echo "Logs: $RUNTIME_DIR"
