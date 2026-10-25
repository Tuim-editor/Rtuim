#!/usr/bin/env bash
set -euo pipefail

VERSION="${VERSION:-$(sed -n 's/^version = "\([^"]*\)"/\1/p' "$(dirname "${BASH_SOURCE[0]}")/Cargo.toml" | head -1)}"
COMMIT_SHA="${COMMIT_SHA:-$(git rev-parse --short HEAD 2>/dev/null || echo unknown)}"
NEOVIM_VERSION="${NEOVIM_VERSION:-v0.12.4}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$ROOT"
APP_DIR="${APP_DIR:-$ROOT/Tuim.AppDir}"
OUTPUT_DIR="${OUTPUT_DIR:-$ROOT}"
WORK_DIR="$(mktemp -d "${TMPDIR:-/tmp}/tuim-appimage.XXXXXX")"
trap 'rm -rf "$WORK_DIR"' EXIT

rm -rf "$APP_DIR"
mkdir -p "$APP_DIR/usr/bin" "$APP_DIR/usr/share/applications" "$APP_DIR/usr/share/metainfo" "$APP_DIR/usr/share/icons/hicolor/scalable/apps" "$OUTPUT_DIR"

[[ "$(uname -s)/$(uname -m)" == Linux/x86_64 ]] || { echo "Build this AppImage on Linux x86-64." >&2; exit 1; }
echo "Building Rust Tuim $VERSION ($COMMIT_SHA) for Linux x86-64..."
cargo build --locked --release --manifest-path "$ROOT/Cargo.toml" --target-dir "$ROOT/target"
actual_version=$("$ROOT/target/release/tuim" --version | awk '{print $2}')
[[ "$VERSION" == "$actual_version" ]] || { echo "VERSION must match Cargo.toml ($actual_version)." >&2; exit 1; }
cp "$ROOT/target/release/tuim" "$APP_DIR/usr/bin/tuim"

if [[ -n "${NEOVIM_SOURCE_DIR:-}" ]]; then
    echo "Using Neovim from $NEOVIM_SOURCE_DIR"
    cp "$NEOVIM_SOURCE_DIR/bin/nvim" "$APP_DIR/usr/bin/nvim"
    cp -R "$NEOVIM_SOURCE_DIR/share/nvim" "$APP_DIR/usr/share/"
else
    echo "Downloading bundled Neovim $NEOVIM_VERSION..."
    archive="$WORK_DIR/nvim-linux-x86_64.tar.gz"
    curl -fL --retry 3 -o "$archive" \
        "https://github.com/neovim/neovim/releases/download/$NEOVIM_VERSION/nvim-linux-x86_64.tar.gz"
    tar -xzf "$archive" -C "$WORK_DIR"
    cp "$WORK_DIR/nvim-linux-x86_64/bin/nvim" "$APP_DIR/usr/bin/nvim"
    cp -R "$WORK_DIR/nvim-linux-x86_64/share/nvim" "$APP_DIR/usr/share/"
fi

cp "$ROOT/packaging/AppRun" "$APP_DIR/AppRun"
cp "$ROOT/packaging/tuim.desktop" "$APP_DIR/io.github.rouboufy.tuim.desktop"
cp "$ROOT/packaging/tuim.desktop" "$APP_DIR/usr/share/applications/io.github.rouboufy.tuim.desktop"
cp "$ROOT/packaging/tuim.svg" "$APP_DIR/tuim.svg"
cp "$ROOT/packaging/tuim.svg" "$APP_DIR/usr/share/icons/hicolor/scalable/apps/tuim.svg"
cp "$ROOT/packaging/tuim.appdata.xml" "$APP_DIR/usr/share/metainfo/io.github.rouboufy.tuim.appdata.xml"
chmod 755 "$APP_DIR/AppRun" "$APP_DIR/usr/bin/tuim" "$APP_DIR/usr/bin/nvim"

cat >"$APP_DIR/VERSION.txt" <<EOF
Tuim version: $VERSION
Git commit: $COMMIT_SHA
Bundled Neovim: $NEOVIM_VERSION
Architecture: x86_64-unknown-linux-gnu
EOF

if [[ "${APPIMAGE_APPDIR_ONLY:-0}" == 1 ]]; then
    echo "Prepared AppDir at $APP_DIR"
    exit 0
fi

linuxdeploy="$WORK_DIR/linuxdeploy-x86_64.AppImage"
curl -fL --retry 3 -o "$linuxdeploy" \
    "https://github.com/linuxdeploy/linuxdeploy/releases/download/continuous/linuxdeploy-x86_64.AppImage"
chmod 755 "$linuxdeploy"

echo "Packaging AppImage without requiring FUSE..."
rm -f "$ROOT/Tuim-$VERSION-x86_64.AppImage"
ARCH=x86_64 LINUXDEPLOY_OUTPUT_VERSION="$VERSION" "$linuxdeploy" --appimage-extract-and-run --appdir "$APP_DIR" --output appimage
generated="$ROOT/Tuim-$VERSION-x86_64.AppImage"
[[ -f "$generated" ]] || { echo "linuxdeploy did not create $generated" >&2; exit 1; }
output="$OUTPUT_DIR/Tuim-$VERSION-x86_64.AppImage"
if [[ "$generated" != "$output" ]]; then mv "$generated" "$output"; fi
chmod 755 "$output"
(cd "$OUTPUT_DIR" && sha256sum "$(basename "$output")" > "$(basename "$output").sha256")
echo "Created $output and $output.sha256"
