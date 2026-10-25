#!/usr/bin/env bash
# Build a native Rust release bundle. Run on the destination OS/architecture.
set -euo pipefail
ROOT=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
cd "$ROOT"
case "$(uname -s)" in Linux) platform=linux ;; Darwin) platform=macos ;; *) echo "Use Linux or macOS." >&2; exit 1 ;; esac
case "$(uname -m)" in x86_64) arch=x86_64; nvim_arch=x86_64 ;; aarch64|arm64) arch=aarch64; nvim_arch=arm64 ;; *) echo "Unsupported architecture." >&2; exit 1 ;; esac
asset="tuim-$platform-$arch"
output="${OUTPUT_DIR:-$ROOT/dist}"
work=$(mktemp -d "${TMPDIR:-/tmp}/tuim-bundle.XXXXXX")
trap 'rm -rf "$work"' EXIT
cargo build --locked --release --manifest-path "$ROOT/Cargo.toml" --target-dir "$ROOT/target"
version=$("$ROOT/target/release/tuim" --version | awk '{print $2}')
if [[ -n "${VERSION:-}" && "$VERSION" != "$version" ]]; then
    echo "VERSION must match Cargo.toml ($version)." >&2; exit 1
fi
bundle="$work/$asset"
mkdir -p "$bundle/bin" "$bundle/lib/tuim" "$output"
cp packaging/tuim-launcher "$bundle/bin/tuim"
chmod 755 "$bundle/bin/tuim"
cp target/release/tuim "$bundle/lib/tuim/tuim"
cp src/nvim/tuim_init.lua "$bundle/lib/tuim/tuim_init.lua"
if [[ -n "${NEOVIM_SOURCE_DIR:-}" ]]; then
    mkdir -p "$bundle/lib/tuim/nvim/bin" "$bundle/lib/tuim/nvim/share"
    cp "$NEOVIM_SOURCE_DIR/bin/nvim" "$bundle/lib/tuim/nvim/bin/"
    cp -R "$NEOVIM_SOURCE_DIR/share/nvim" "$bundle/lib/tuim/nvim/share/"
else
    nvim_asset="nvim-$platform-$nvim_arch"
    curl -fL --retry 3 -o "$work/nvim.tar.gz" "https://github.com/neovim/neovim/releases/download/${NEOVIM_VERSION:-v0.12.4}/$nvim_asset.tar.gz"
    tar -xzf "$work/nvim.tar.gz" -C "$work"
    mv "$work/$nvim_asset" "$bundle/lib/tuim/nvim"
fi
"$bundle/bin/tuim" --version
"$bundle/lib/tuim/nvim/bin/nvim" --version | head -1
printf 'Tuim %s\nPlatform: %s/%s\n' "$version" "$platform" "$arch" > "$bundle/VERSION.txt"
cp LICENSE "$bundle/LICENSE"
tar -C "$work" -czf "$output/$asset.tar.gz" "$asset"
python3 - "$output/$asset.tar.gz" <<'CHECKSUM'
import hashlib, pathlib, sys
p = pathlib.Path(sys.argv[1])
p.with_suffix(p.suffix + '.sha256').write_text(hashlib.sha256(p.read_bytes()).hexdigest() + '  ' + p.name + '\n')
CHECKSUM
printf 'Created %s\n' "$output/$asset.tar.gz"
