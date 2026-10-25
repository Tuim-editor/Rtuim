# Rtuim wiki

Rtuim is the MIT-licensed Rust rewrite of [Tuim](https://github.com/Tuim-editor/Tuim),
with an isolated Neovim editing
engine. It brings files, search, Git, native shell panels, language tools,
plugins and coding assistants into one interface.

- [Installation](Installation.md): source builds, release bundles and removal.
- [Keybindings](Keybindings.md): controls verified against Rust and Lua.
- [Features](Features.md): workspace modes, terminals, tools and themes.
- [Troubleshooting](Troubleshooting.md): recovery, clipboard and diagnostics.
- [Usage guide](../usage.md) and [architecture](../architecture.md).
- [Migration audit](../migration-audit.md): local validation and remaining issues.

These are local wiki sources. The live GitHub wiki has not been changed.
Normal mode uses Vim editing; IDE uses modeless editing; F11 toggles Zen.
Tuim uses its own XDG config, data, state and cache directories and does not
load the user's regular Neovim init.
