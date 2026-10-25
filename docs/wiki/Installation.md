# Installation

[Rtuim](https://github.com/Tuim-editor/Rtuim) is the MIT-licensed Rust rewrite
of Tuim. Build from source until Rtuim release packages are published.
The executable and configuration directories still use the `tuim` name.
See [migration status](../releases/rust-migration.md).

## Build from the Rust checkout

Requires a current stable Rust toolchain with Cargo, a C linker, Git, and
Neovim 0.12 or newer on PATH. Python 3 is used by Extensions, ripgrep by project
search, and a C compiler plus Tree-sitter CLI by parser installation.

```bash
cargo build --locked --release
./target/release/tuim
cargo test --locked
```

For a user-local source installation:

```bash
bash setup.sh --source --yes
```

This uses your installed Rust/Cargo, builds the release executable, provisions
private Neovim if needed, and links `~/.local/bin/tuim`. Add that directory to
PATH. A source install with `--no-plugins` still needs Rust and a linker.

## Installer options

[`setup.sh`](../../setup.sh) defaults to downloading the latest published native
bundle and verifying SHA-256 checksums. Release bundles include private Neovim
under Tuim's `runtime/`; source setup can provision Neovim under `tools/nvim/`.

- `--source`: build from a checkout using installed Rust/Cargo.
- `--no-plugins`: skip plugin/parser bootstrap and parser build dependencies.
- `--dry-run`: display the plan without changing files or packages.
- `--yes`: consent to missing system dependency installation without prompting.

Plugin/parser bootstrap needs network access. Existing settings are preserved.
No desktop shortcuts are installed into host directories by this script.

## Packages

`bash scripts/build_bundle.sh` builds a native bundle for the current Linux or
macOS architecture. `bash build_appimage.sh` creates a Linux x86-64 AppImage
named `Tuim-<Cargo version>-x86_64.AppImage`. See [packaging](../appimage.md).
Linux is locally tested; macOS/ARM64 and WSL still require native validation.

## Remove an installation

```bash
bash uninstall.sh --binary
bash uninstall.sh --help
```

The first command removes only the launcher after confirmation. `--all` removes
configuration, data, caches, logs and sessions. `--plugins` selectively removes
installed plugin artifacts (`lazy/`, `mason/`, `site/`, `plugin_inventory.json`)
while strictly preserving user plugin configurations (`plugin_configs/`) and
preferences. `--settings` removes configuration and `settings.json` (and backups).
