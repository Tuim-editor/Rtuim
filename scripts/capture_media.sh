#!/usr/bin/env bash
# Compatibility entry point for the current Rust screenshot workflow.
set -euo pipefail
root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$root"
cargo build --locked --release
exec uv run --with pillow --with pyte python scripts/capture_screenshots.py "$@"
