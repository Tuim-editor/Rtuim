# Current application media

The obsolete Zig-era SVG and asciinema fixtures have been removed. Current
Rust UI screenshots and their terminal text are in [screenshots](../screenshots/README.md).
The README and website share those real captures.

Run `bash scripts/capture_media.sh` to refresh them. The wrapper delegates to
`scripts/capture_screenshots.py`; it needs uv, tmux, Neovim and fontconfig.
