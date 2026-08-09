#!/usr/bin/env bash
# Stop only processes whose recorded PID and command match this local stack.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUNTIME_DIR="${POC_RUNTIME_DIR:-$PROJECT_ROOT/.local/runtime}"

stop_owned_process() {
    local pid_file="$1"
    local expected_command="$2"
    local label="$3"
    if [[ ! -f "$pid_file" ]]; then
        return 0
    fi

    local recorded_pid
    recorded_pid="$(tr -cd '0-9' < "$pid_file")"
    if [[ -z "$recorded_pid" ]] || ! kill -0 "$recorded_pid" 2>/dev/null; then
        rm -f "$pid_file"
        return 0
    fi

    local command_line
    command_line="$(ps -p "$recorded_pid" -o command= 2>/dev/null || true)"
    if [[ "$command_line" != *"$expected_command"* ]]; then
        echo "refusing to stop pid $recorded_pid: it is not $label" >&2
        return 1
    fi

    kill "$recorded_pid"
    for _attempt in $(seq 1 100); do
        if ! kill -0 "$recorded_pid" 2>/dev/null; then
            break
        fi
        sleep 0.05
    done
    if kill -0 "$recorded_pid" 2>/dev/null; then
        echo "$label pid $recorded_pid did not stop after SIGTERM" >&2
        return 1
    fi
    rm -f "$pid_file"
    echo "Stopped $label (pid $recorded_pid)"
}

stop_owned_process "$RUNTIME_DIR/ephemeral-validator.pid" "ephemeral-validator" "ephemeral-validator"
stop_owned_process "$RUNTIME_DIR/surfpool.pid" "surfpool" "Surfpool"
