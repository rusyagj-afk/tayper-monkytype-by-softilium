#!/usr/bin/env bash
set -e

DIR="$(dirname "$(readlink -f "$0")")"
BIN="$DIR/target/release/softilium"

if [ ! -f "$BIN" ]; then
    echo "[softilium] Сборка..."
    cargo build --release --manifest-path="$DIR/Cargo.toml"
fi

exec "$BIN" "$@"
