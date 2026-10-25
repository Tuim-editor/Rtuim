# Rust bundles and AppImage packaging

Native bundles contain the Rust executable, launcher and private Neovim runtime.
Build on the destination Linux or macOS architecture:

```bash
bash scripts/build_bundle.sh
```

Output is `dist/tuim-<linux|macos>-<x86_64|aarch64>.tar.gz` plus its SHA-256 file.
`NEOVIM_VERSION` defaults to `v0.12.4`. For a local smoke bundle, set
`NEOVIM_SOURCE_DIR` to a prefix containing `bin/nvim` and `share/nvim` (for
example `/usr`). A distribution-provided Neovim may depend on host libraries;
that option alone does not produce a generally portable package.

The GitHub release workflow builds on native Linux x86-64/ARM64 and macOS
Intel/Apple Silicon runners. `VERSION`, when supplied, must match `Cargo.toml`.
The Rust executable embeds its Lua/Python runtime assets at compile time.

## Linux x86-64 AppImage

```bash
bash build_appimage.sh
bash tests/appimage_smoke.sh Tuim.AppDir
./Tuim-0.4.0-x86_64.AppImage --appimage-extract-and-run --version
sha256sum -c Tuim-0.4.0-x86_64.AppImage.sha256
```

Requires stable Rust/Cargo, a C linker, Python 3, curl, tar and Linux x86-64.
The builder bundles Neovim and uses linuxdeploy to collect shared libraries.
Use an older compatible Linux build host for public releases: locally building
on a recent distribution does not guarantee support for older glibc versions.
The workflow uses Ubuntu 22.04. `OUTPUT_DIR` changes the artifact directory;
`APPIMAGE_APPDIR_ONLY=1` prepares just the AppDir.

The AppImage includes desktop metadata, icons and `VERSION.txt`. It stores
user data in Tuim's normal isolated XDG directories outside the image. The
release workflow also creates the updater asset `Tuim-linux-x86_64.AppImage`
and release-wide `SHA256SUMS`.

## Validation limits

Validate the Rust packages on each destination before publishing. See
[terminal compatibility](terminal-compatibility.md) for tested platforms and
outstanding validation. Local Linux builds inherit the build host's libc baseline.
