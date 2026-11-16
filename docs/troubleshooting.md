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
Enter a summary and optional reproduction details, then click **Send** or press
`Ctrl+S`. No GitHub account, token, configuration, or external upload tool is
needed. Official releases include the reporting service address.

Sending creates a **public GitHub issue** with basic runtime diagnostics (version,
OS, architecture, theme, mode, terminal capabilities/dimensions, and emitted-byte
count). No editor files or logs are attached. The home path and recognized token
patterns are redacted, but this cannot detect every secret: remove private details
before sending. `Escape` closes the dialog; an upload already in progress continues
in the background while TUIM remains open. After a failure, `Enter` returns to the
preserved report for editing and `Ctrl+S` retries.

If the service is temporarily unavailable, check your connection and retry later,
or use [GitHub issues](https://github.com/Tuim-editor/Rtuim/issues).
Maintainers can find the one-time service deployment and release configuration in
[the gateway guide](../reporting/README.md). `TUIM_BUG_REPORT_URL` remains available
as a runtime override for testing or self-hosted deployments; normal users do not
need to set it.
