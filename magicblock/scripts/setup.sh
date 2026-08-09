#!/usr/bin/env bash
# Build the Pinocchio program and pinned official local MagicBlock dependencies.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TOOLS_DIR="$PROJECT_ROOT/.local-tools"
DLP_DIR="$TOOLS_DIR/delegation-program"
DLP_COMMIT="27f7fd8178630b88e2be45d92b174b8f2ef4661e"
NOOP_VERSION="1.0.0"
NOOP_DIR="$TOOLS_DIR/spl-noop-$NOOP_VERSION"
NOOP_ARCHIVE="$TOOLS_DIR/spl-noop-$NOOP_VERSION.crate"
NOOP_SHA256="1c3bc351f7543a46f6807c231fc29ef2c4912c79bd6a4fb7d038cba6836f0fd7"
VALIDATOR_VERSION="0.13.19"
VALIDATOR_BIN="$TOOLS_DIR/ephemeral-validator"

for required_command in cargo cargo-build-sbf curl git nc npm solana-keygen tar; do
    if ! command -v "$required_command" >/dev/null 2>&1; then
        echo "missing required command: $required_command" >&2
        exit 1
    fi
done

mkdir -p "$TOOLS_DIR" "$PROJECT_ROOT/target/deploy"

sha256_file() {
    if command -v sha256sum >/dev/null 2>&1; then
        sha256sum "$1" | awk '{print $1}'
    else
        shasum -a 256 "$1" | awk '{print $1}'
    fi
}

echo "Building Pinocchio stream program for SBF"
cargo build-sbf \
    --manifest-path "$PROJECT_ROOT/program/Cargo.toml" \
    --sbf-out-dir "$PROJECT_ROOT/target/deploy"
cp "$PROJECT_ROOT/keys/program-keypair.json" \
    "$PROJECT_ROOT/target/deploy/magicblock_usdc_stream-keypair.json"

if [[ ! -d "$DLP_DIR/.git" ]]; then
    if [[ -e "$DLP_DIR" ]]; then
        echo "$DLP_DIR exists but is not the managed DLP checkout" >&2
        exit 1
    fi
    echo "Cloning official delegation program v3.1.0"
    git clone --branch v3.1.0 --depth 1 \
        https://github.com/magicblock-labs/delegation-program.git "$DLP_DIR"
fi

actual_dlp_commit="$(git -C "$DLP_DIR" rev-parse HEAD)"
if [[ "$actual_dlp_commit" != "$DLP_COMMIT" ]]; then
    echo "unexpected DLP revision: $actual_dlp_commit (expected $DLP_COMMIT)" >&2
    exit 1
fi

echo "Building official delegation program v3.1.0 for SBF"
(
    cd "$DLP_DIR"
    cargo build-sbf
)

# The v0.13.19 committor appends the canonical SPL Noop instruction to make
# otherwise-identical settlement transactions unique. Offline Surfpool cannot
# clone it from devnet, so build the checksum-pinned official crate locally.
if [[ ! -f "$NOOP_ARCHIVE" ]]; then
    echo "Downloading official SPL Noop v$NOOP_VERSION"
    curl --fail --location --retry 3 \
        "https://static.crates.io/crates/spl-noop/spl-noop-$NOOP_VERSION.crate" \
        --output "$NOOP_ARCHIVE"
fi
actual_noop_sha256="$(sha256_file "$NOOP_ARCHIVE")"
if [[ "$actual_noop_sha256" != "$NOOP_SHA256" ]]; then
    echo "SPL Noop archive checksum mismatch" >&2
    exit 1
fi
if [[ ! -f "$NOOP_DIR/Cargo.toml" ]]; then
    if [[ -e "$NOOP_DIR" ]]; then
        echo "$NOOP_DIR exists but is not the managed SPL Noop source" >&2
        exit 1
    fi
    tar -xzf "$NOOP_ARCHIVE" -C "$TOOLS_DIR"
fi
echo "Building official SPL Noop v$NOOP_VERSION for SBF"
cargo build-sbf \
    --manifest-path "$NOOP_DIR/Cargo.toml" \
    --sbf-out-dir "$PROJECT_ROOT/target/deploy"

case "$(uname -s)-$(uname -m)" in
    Darwin-arm64)
        validator_asset="ephemeral-validator-darwin-arm64"
        validator_sha256="276e0fa714f36ca433d1275e62725f313f07d75715316ed4f6dfa4f4014ec202"
        ;;
    Darwin-x86_64)
        validator_asset="ephemeral-validator-darwin-x64"
        validator_sha256="e0c09f15d16d1fec718775791daa16fd4404d2d180d4f9914d8821cbcc3ce1e3"
        ;;
    Linux-aarch64|Linux-arm64)
        validator_asset="ephemeral-validator-linux-arm64-glibc"
        validator_sha256="fa7b949e2c95f3321af4b918629035ea1b357bbcc9cc974118df67afb34e7d36"
        ;;
    Linux-x86_64)
        validator_asset="ephemeral-validator-linux-x64-glibc"
        validator_sha256="50c996fbbf4d7b4b019fa95ad22234c487d2f7c87798d78f77bbfbcd21de5aa4"
        ;;
    *)
        echo "unsupported validator platform: $(uname -s) $(uname -m)" >&2
        exit 1
        ;;
esac

if [[ -f "$VALIDATOR_BIN" ]]; then
    actual_validator_sha256="$(sha256_file "$VALIDATOR_BIN")"
    if [[ "$actual_validator_sha256" != "$validator_sha256" ]]; then
        echo "validator checksum mismatch at $VALIDATOR_BIN" >&2
        exit 1
    fi
else
    validator_download="$VALIDATOR_BIN.download.$$"
    trap 'rm -f "$validator_download"' EXIT
    echo "Downloading official ephemeral-validator v$VALIDATOR_VERSION"
    curl --fail --location --retry 3 \
        "https://github.com/magicblock-labs/magicblock-validator/releases/download/v$VALIDATOR_VERSION/$validator_asset" \
        --output "$validator_download"
    actual_validator_sha256="$(sha256_file "$validator_download")"
    if [[ "$actual_validator_sha256" != "$validator_sha256" ]]; then
        echo "downloaded validator checksum mismatch" >&2
        exit 1
    fi
    mv "$validator_download" "$VALIDATOR_BIN"
    trap - EXIT
fi
chmod 755 "$VALIDATOR_BIN"

program_address="$(solana-keygen pubkey "$PROJECT_ROOT/keys/program-keypair.json")"
if [[ "$program_address" != "J6JPeaFMpp9hoha6KGfG2tWTWhAqdtJtWJwrNYDW9SFx" ]]; then
    echo "checked-in program keypair does not match the compiled program id" >&2
    exit 1
fi

echo "Checking every Rust workspace member"
cargo check --workspace --manifest-path "$PROJECT_ROOT/Cargo.toml"

echo "Installing TS client dependencies"
(cd "$PROJECT_ROOT" && npm install)

echo "Local artifacts are ready"
