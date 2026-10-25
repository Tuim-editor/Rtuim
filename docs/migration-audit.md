# Rust migration audit — 2026-10-22

All work is local. No release, website, GitHub wiki or Git branch was published.
Antigravity (Gemini 3.8 Flash) in the existing tmux pane reviewed the changes
implemented by Codex. `herdr` was unavailable, so review handoffs used tmux.

## Delivered & Validated (v0.4.0)

- Rewritten Rust README and refreshed static website, with ten real screenshots
  shared across both and terminal text captures for review.
- Cargo installers, updater, AppImage builder, native bundle script, CI/release
  workflows, and Rust executable paths in test/capture/profile scripts.
- Corrected architecture, usage, compatibility guides, and local wiki sources.
- Version bumped from 0.3.0 to **0.4.0** in `Cargo.toml` and lockfile.
- Rebuilt Linux x86-64 executable (`bin/tuim`, `target/release/tuim`), native
  tarball and AppImage. Distributables and `SHA256SUMS` are in `dist/`.

## Local validation

Host: Fedora Linux x86-64, rustc/Cargo 1.99.0. UI captures and core tests use
Neovim 0.12.5; packaged artifacts contain the upstream Neovim 0.12.4 runtime.

Passed:

- 28 Rust unit tests and release build.
- Mocked bug report lifecycle tests: non-looping redaction of repeated, empty,
  and UTF-8 secrets; argument-literal upload via tracked child process without
  shell interpolation; process reaping and cleanup on drop.
- PTY integration in Normal, IDE and Zen; controlling-terminal input test.
- Native bottom/right terminal tests: leader mappings, independent shells,
  keyboard and mouse resize, forward/reverse focus, F12, mode transitions,
  compact layouts down to 8×4, close and cleanup.
- Terminal clipboard tests: copy/paste variants, Unicode/multiline text,
  clipboard provider, cross-pane/editor sharing and Zen.
- Command palette tests: recording, collisions, persistence, mouse, buffer
  switching/closing, unsaved cancellation and compact layouts.
- Default-runtime theme/selection/persistence checks (available parser checks
  only; full parser installation was not performed).
- Selective uninstaller fixture matrix, including custom XDG paths and preservation.
- Installer fixture suite, package-manager plans and symlinked launcher test.
- AppDir terminal smoke suite, AppStream metadata validation, final AppImage
  extraction/version check, and native bundle executable identity check.
- Static site links/metadata, ten screenshot/text pairs, shell syntax, and
  recorded Rust performance schema; desktop and mobile Chromium visual review.
- Shellcheck and YAML parsing for the migrated scripts and workflows.
- Local distribution SHA-256 verification.

The performance sample has two iterations with no warmup. Plugin initialization
is opt-in and was not measured. These results establish no speedup over Zig.
Cross-platform workflows were prepared but not run on remote CI. macOS and
ARM64 binaries require their native runners; WSL and older Linux distributions
remain unverified. Local Linux artifacts inherit this build host's libc baseline.

## Audit Findings & Resolutions

The following discrepancies identified during migration are resolved in v0.4.0:

1. **Bug Report Redaction & Submission:**
   `redact_text` in `src/tui/widgets/bug_report.rs` now scans using a forward
   cursor, eliminating the infinite loop when secret tokens or
   already redacted strings are encountered. Report submission runs `curl`
   directly without `sh -c` shell interpolation, managing an active `Child`
   handle whose `Drop` implementation kills and reaps in-flight uploads and
   removes private temporary files.
2. **Reverse Focus Navigation (`Shift+F6`):**
   `decode_key_sequence` decodes `\x1b[17;2~` as `<S-F6>`. Event dispatch
   cycles backward (`focus_prev`) through visible regions with positive
   geometry (`Editor` → `VerticalTerminal` → `Drawer` → `Sidebar` → `Editor`).
3. **Keyboard Panel Resizing (`Alt+Arrow`):**
   Decodes `\x1b[1;3A..D` as `<M-Up>`, `<M-Down>`, `<M-Right>`, `<M-Left>`.
   `Alt+Left`/`Alt+Right` adjusts the vertical terminal divider (if focused or
   sidebar is collapsed) or sidebar divider; `Alt+Up`/`Alt+Down` adjusts the
   drawer divider. Visible Zen panes resize cleanly with bounds clamping.
4. **Hardcoded Aliases Removed:**
   Removed hardcoded `<C-b>` and `<F6>` aliases in `src/tui/events.rs`.
   Rebinding in `settings.json` takes immediate effect without being shadowed.
5. **Real Opt-In Vim-Safe Preset:**
   Factory defaults and `Keybindings::default()` retain standard Ctrl bindings
   (`Ctrl+E`, `Ctrl+T`, `Ctrl+N`, `Ctrl+P`, `Ctrl+S`, `Ctrl+Q`), preserving
   existing saved shortcuts. Applying the Vim-safe preset (`v` in Settings)
   switches to `Alt+E`, `Alt+T`, `Alt+N`, `Alt+P`, `Alt+Q`, and `F2`, freeing
   Vim motions and completion keys.
6. **Bug Report Shortcut (`F12`):**
   `report_bug` defaults to `<F12>` in factory presets. If a user previously
   saved an empty string in `settings.json`, that preference is preserved.
7. **Window Split / Close vs AI Collisions:**
   Removed conflicting `<M-s>` and `<M-c>` AI mappings from `tuim_init.lua`.
   `<M-s>` performs `:split` and `<M-c>` performs `:close`. AI context actions
   are mapped to `Ctrl+Alt+S`/`F`/`C` and leader chords (`<leader>as/af/ac`).
8. **Native Explorer via `Space e`:**
   In Normal mode, `<leader>e` calls `tuim_toggle_explorer` over RPC, toggling
   the native Workspace Explorer without depending on unbundled Neo-tree.
9. **Git Staging Shortcuts:**
   `+` explicitly stages the selected change (`stage_file`), `-` explicitly
   unstages (`unstage_file`), including before the first commit, and `Space`/`s`
   toggles staging state. Working files are preserved.
10. **Selective Uninstaller Scope:**
    In `uninstall.sh`, `--plugins` removes installed plugin assets (`lazy/`,
    `mason/`, `site/`, `plugin_inventory.json`) while strictly preserving
    user configurations (`plugin_configs/`) and preferences. `--settings`
    removes `${XDG_CONFIG_HOME}/tuim` and `${XDG_DATA_HOME}/tuim/settings.json`.
    `--all` removes all Tuim directories.

## Remaining release work

- This checkout has no commits or Git remotes. Connect the intended upstream
  before publishing; retained public links were not synchronized with it.
- The corrected wiki lives in `docs/wiki/`. The live GitHub wiki was not fetched
  or changed and must be reconciled before publication.
- Report transport tests use local mocks; no report was sent to the live service.
- macOS, ARM64, WSL and older Linux validation remain outstanding.

## Build references

Cargo commands follow the [Cargo build documentation](https://doc.rust-lang.org/cargo/commands/cargo-build.html).
Native CI platform labels follow [GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
