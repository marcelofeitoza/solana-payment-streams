#!/usr/bin/env bash
# Execute the complete 1,000,000 USDC / 100 USDC local demonstration.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
STATE_DIR="${POC_STATE_DIR:-$PROJECT_ROOT/.local/demo}"

client() {
    (
        cd "$PROJECT_ROOT"
        BASE_RPC=http://127.0.0.1:9900 \
        ER_RPC=http://127.0.0.1:17899 \
        STATE_DIR="$STATE_DIR" \
            npx tsx client/index.ts "$@"
    )
}

client init-usdc
client mint --amount 1000000
client balances
client init-stream \
    --total 1000000 \
    --chunk 100 \
    --interval-ms 1 \
    --fee-reserve-lamports 5000000000
client balances
client schedule
client watch --timeout-seconds 900
client balances
