# Rtuim — the Rust rewrite of Tuim

We recoded Tuim's frontend from Zig to Rust. The Rust version is called
[Rtuim](https://github.com/Tuim-editor/Rtuim), with Neovim still providing the
editing engine. Like the [original Tuim](https://github.com/Tuim-editor/Tuim),
Rtuim is distributed under the [MIT license](../../LICENSE).

`Cargo.toml` reports 0.4.0. This repository is a source preview; packaged
Rtuim releases are not yet published. The executable and configuration
directories retain the `tuim` name for compatibility.

- Cargo replaces the Zig build and installer toolchain.
- Native release bundles build on Linux x86-64/ARM64 and macOS Intel/Apple
  Silicon runners. Tag and Cargo versions must match.
- Bottom and right-side terminals use direct PTYs with persistent shell sessions.
- The current workspace includes project search, Git changes/history/branches,
  Extensions, language tools, assistant context actions, and a worktree orchestrator.
- README, website, wiki sources, capture scripts, and test executable paths
  describe the Rust tree. Screenshots come from the actual Rust executable.

The frontend uses Unix APIs. Native Windows support is not implemented; WSL
and macOS need validation of this rewrite. Previous Zig performance and
compatibility results do not establish Rust behavior.

See [usage](../usage.md), [local wiki](../wiki/Home.md), and
[packaging](../appimage.md). Nothing is published by updating these files.
