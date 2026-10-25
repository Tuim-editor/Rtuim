# Terminal compatibility

These results describe the Rust rewrite. Previous Zig-era platform observations
are not evidence for the new executable.

| Environment | Rust validation |
| --- | --- |
| Fedora Linux x86-64, Neovim 0.12.5 | Unit tests, PTY/controlling-terminal tests, and isolated tmux UI tests passed locally on 2026-10-22 |
| Linux x86-64 AppDir with Neovim 0.12.4 | PTY suite and packaged version check passed locally |
| macOS Intel / Apple Silicon | Native CI jobs prepared; not executed during this refresh |
| Linux ARM64 | Native CI job prepared; not executed during this refresh |
| Windows Terminal / WSL | Unverified; manual self-hosted workflow available |
| Native Windows | Unsupported: the frontend currently uses Unix APIs |

The tmux tests exercised command binding/persistence, bottom and right PTYs,
clipboard variants, Unicode, Zen, resizing and cleanup. They do not establish
compatibility with every outer terminal emulator. Full plugin/parser installation
and live bug-report delivery still require separate validation.

## Capability detection

[`capabilities.rs`](../src/tui/capabilities.rs) infers capabilities from the
terminal environment. It recognizes TERM/COLORTERM, several TERM_PROGRAM
values, and VTE, Konsole, Windows Terminal, tmux, SSH and WSL markers.
Recognition is not an end-to-end test of each emulator.

Dumb terminals disable mouse and bracketed paste. Linux console profiles disable
mouse and true color but retain bracketed paste. tmux, SSH and WSL use conservative
modifier assumptions. Fonts, mouse forwarding and clipboard providers also depend
on the outer terminal and transport.

Use portable symbols in Settings if icons are unavailable. System clipboard
integration needs a suitable provider; otherwise use Tuim's internal clipboard.
F1 and the mouse menus provide access when a modified key is unavailable.

## Reproduce

```bash
cargo build --locked
python3 tests/controlling_tty.py
bash tests/terminal_compat.sh host
bash tests/terminal_compat.sh tmux
```

`TUIM_TEST_BINARY` selects another executable. The optional `ssh` mode requires
local OpenSSH server tools and exercises a forced remote PTY. Record the host OS,
terminal, Neovim version, date, exact command and result for each new observation.
