# Rust Tuim screenshots

These images are real terminal grids captured from `target/release/tuim`.
No application UI is fabricated. Website and README use the same WebP files;
matching `.txt` files preserve visible terminal text for review.

```bash
cargo build --locked --release
uv run --with pillow --with pyte python scripts/capture_screenshots.py
```

Requires Neovim, tmux, fontconfig, a monospace font, and an existing Tuim data
folder with `lazy/` and `site/`. Use `--data-dir` for a non-default location.
The capture script copies the plugin tree into temporary XDG directories,
but disables plugin execution for deterministic offline screenshots. It does
not demonstrate Treesitter, completion, or active language servers.

A disposable Git project contains Rust source and Cargo metadata. Captures use
VS Code Dark Modern, portable symbols, and a 132 × 38 cell viewport rendered
to 1584 × 950 pixels. Existing user settings are not changed. The set covers
Explorer, workspace, shell, Git, settings, Extensions, AI, worktrees, IDE and Zen.

Regenerate and visually inspect the images after UI changes. Previously stored
Zig SVG/asciinema fixtures have been removed; the current captures replace them.
