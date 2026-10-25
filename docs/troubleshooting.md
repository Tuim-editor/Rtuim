# Troubleshooting

## Plugins

Run `TUIM_DISABLE_PLUGINS=1 tuim` to skip plugin loading without deleting plugin
data. Repair plugin configuration through Extensions or the files in Tuim's
`plugin_configs/` directory, then restart normally. See
[plugin compatibility](plugin-compatibility.md).

## Neovim or build failure

Check `cargo --version`, `rustc --version` and `nvim --version`. Manual source
builds need Neovim 0.12+ on PATH; native bundles and AppImages supply private
Neovim through their launchers. Run from a checkout containing `Cargo.toml`.

## Clipboard and shortcuts

Enable **Settings → General → System Clipboard** to share with other apps.
A host provider such as wl-clipboard, xclip/xsel or macOS pbcopy/pbpaste may be
needed. Otherwise the editor and native terminals share Tuim's internal clipboard.
Select terminal text by dragging, then press Ctrl+C or Ctrl+Insert; release
alone does not copy. Ctrl+C without selection interrupts the running program.

Use [the keybinding reference](keybindings.md). F1, F6 / Shift+F6, Alt+Arrows,
and mouse controls offer navigation and panel resizing when the outer terminal
cannot distinguish modified keys. Shortcut customizations can be configured in
Settings (`F1` → Settings → Keybindings).

## Fonts and colors

Disable **Settings → Appearance → Use Nerd Fonts** to use portable symbols.
Color and modifier behavior depends on the terminal and tmux/SSH transport.
See [terminal compatibility](terminal-compatibility.md).

## Diagnostics and reporting

Use `:messages` in Normal mode for Neovim messages. Run `tuim --diagnostics` to
write performance diagnostics on exit to `$XDG_DATA_HOME/tuim/diagnostics.json`
(default `~/.local/share/tuim/diagnostics.json`). To locate Neovim's own log,
use `:lua print(vim.fn.stdpath('log'))`.

Press `F12` or choose **F1 → Report bug** to open the in-app report form.
Ctrl+S advances to the confirmation screen; Enter or y confirms sending, and
Escape or n cancels. Report submission runs shell-free using curl with automatic
child cleanup, and the home path and recognized token patterns are redacted. Review the text
before sending; this is not a guarantee that every possible secret is detected.
The endpoint is configured with `TUIM_BUG_REPORT_URL`. GitHub issues provide
another reporting route.
