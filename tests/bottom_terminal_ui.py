#!/usr/bin/env python3
"""Exercise Tuim's direct PTY shell, theme changes, input, and lifecycle."""
import json
import os
import pathlib
import re
import shlex
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]


def ansi_cells(line):
    fg = bg = None
    cells = []
    for part in re.split(r'(\x1b\[[0-9;]*m)', line):
        if not part.startswith('\x1b['):
            cells.extend((fg, bg) for _ in part)
            continue
        codes = [int(c or 0) for c in part[2:-1].split(';')]
        i = 0
        while i < len(codes):
            code = codes[i]
            if code == 0:
                fg = bg = None
            elif code in (39, 49):
                if code == 39: fg = None
                else: bg = None
            elif code in (38, 48) and i + 1 < len(codes):
                count = 4 if codes[i + 1] == 2 else 2
                color = tuple(codes[i + 1:i + count + 1])
                if code == 38: fg = color
                else: bg = color
                i += count
            i += 1
    return cells


def run():
    with tempfile.TemporaryDirectory(prefix='tuim-native-terminal-') as directory:
        base = pathlib.Path(directory)
        env = {'TUIM_DISABLE_PLUGINS': '1', 'TUIM_SKIP_ONBOARDING': '1', 'SHELL': '/bin/sh', 'TERM': 'xterm-256color'}
        for name in ('config', 'data', 'state', 'cache'):
            (base / name).mkdir()
            env[f'XDG_{name.upper()}_HOME'] = str(base / name)
        (base / 'data/tuim').mkdir()
        (base / 'data/tuim/settings.json').write_text(json.dumps({'mode': 'normal', 'nerd_fonts': False, 'clipboard': False}))
        sample = base / 'sample.txt'
        sample.write_text('editor fixture\n')
        # An application reading raw bytes proves Escape is delivered once,
        # Ctrl-C reaches the child, and shell editing keys are not UI shortcuts.
        (base / 'probe.py').write_text('''import os, pathlib, select, sys, termios, tty
name, expected = sys.argv[1], bytes.fromhex(sys.argv[2])
saved = termios.tcgetattr(0)
try:
    tty.setraw(0)
    if name == 'mouse': os.write(1, b'\\x1b[?1000h\\x1b[?1006h')
    if name == 'wheel': os.write(1, b'\\x1b[?1049h\\x1b[?1h')
    os.write(1, b'\\x1b[?2004hPROBE_READY\\r\\n')
    received = b''
    while len(received) < len(expected):
        if not select.select([0], [], [], 5)[0]: break
        received += os.read(0, len(expected) - len(received))
    pathlib.Path(name).write_bytes(received)
finally:
    os.write(1, b'\\x1b[?2004l')
    if name == 'mouse': os.write(1, b'\\x1b[?1000l\\x1b[?1006l')
    if name == 'wheel': os.write(1, b'\\x1b[?1049l\\x1b[?1l')
    termios.tcsetattr(0, termios.TCSANOW, saved)
''')
        socket = str(base / 'tmux.sock')

        def tmux(*args):
            return subprocess.check_output(['tmux', '-S', socket, *args], text=True)

        def screen(ansi=False):
            return tmux('capture-pane', '-p', *(['-e', '-N'] if ansi else []), '-t', 'ui')

        def wait(predicate, message):
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                if predicate(): return
                time.sleep(.04)
            raise AssertionError(message + '\n' + screen())

        def text(value):
            tmux('send-keys', '-l', '-t', 'ui', value)

        def send(*keys):
            for key in keys:
                tmux('send-keys', '-t', 'ui', key)
                time.sleep(.12)

        def command(value):
            text(value)
            send('Enter')

        def marker(name):
            return (base / name).exists() and (base / name).stat().st_size > 0

        def probe(name, expected, send_input):
            command('clear')
            wait(lambda: 'PROBE_READY' not in screen(), 'Old probe was not cleared')
            command(f'python3 probe.py {name} {expected.hex()}')
            wait(lambda: 'PROBE_READY' in screen(), 'Raw input probe did not start')
            send_input()
            wait(lambda: marker(name), 'Terminal did not deliver input to raw program')
            assert (base / name).read_bytes() == expected, (name, (base / name).read_bytes())

        binary = os.environ.get('TUIM_TEST_BINARY', str(ROOT / 'target/debug/tuim'))
        launch = shlex.join(['env', *(f'{k}={v}' for k, v in env.items()), binary, str(sample)])
        try:
            tmux('new-session', '-d', '-s', 'ui', '-x', '120', '-y', '36', '-c', str(base), launch)
            wait(lambda: 'editor fixture' in screen(), 'Editor did not start')
            app_pid = int(tmux('display-message', '-p', '-t', 'ui', '#{pane_pid}'))
            send('C-t')
            command("PS1='PTY> '")
            command("printf '%s' $$ > shell.pid; export TUIM_NATIVE_SESSION=preserved")
            wait(lambda: marker('shell.pid'), 'Shell did not start')
            shell_pid = int((base / 'shell.pid').read_text())
            parent = int(subprocess.check_output(['ps', '-p', str(shell_pid), '-o', 'ppid='], text=True))
            assert parent == app_pid, 'Bottom shell is still owned by an intermediate process'
            probe('escape', b'\x1b', lambda: send('Escape'))
            probe('controls', b'\x03\x05\x02', lambda: send('C-c', 'C-e', 'C-b'))
            pasted = 'first\nsecond 界'
            probe('paste', ('\x1b[200~' + pasted + '\x1b[201~').encode(),
                  lambda: text('\x1b[200~' + pasted + '\x1b[201~'))
            def mouse_at_panel(button, pressed=True, x=3, y=2):
                lines = screen().splitlines()
                header = next(y for y, line in enumerate(lines) if '[Terminal]' in line)
                panel_x = lines[header].index('[Terminal]') - 2
                text(f'\x1b[<{button};{panel_x + x + 1};{header + y + 2}{"M" if pressed else "m"}')

            probe('mouse', b'\x1b[<0;4;3M\x1b[<0;4;3m',
                  lambda: (mouse_at_panel(0), mouse_at_panel(0, False)))
            probe('wheel', b'\x1bOA' * 3, lambda: mouse_at_panel(64))
            command('clear; printf "COPY_TEXT\\n"')
            wait(lambda: 'COPY_TEXT' in screen(), 'Selection fixture did not render')
            lines = screen().splitlines()
            y, line = next((y, line) for y, line in enumerate(lines) if 'COPY_TEXT' in line)
            x = line.index('COPY_TEXT')
            text(f'\x1b[<0;{x + 1};{y + 1}M\x1b[<32;{x + 9};{y + 1}M\x1b[<0;{x + 9};{y + 1}m')
            send('C-c')
            probe('copy', b'\x1b[200~COPY_TEXT\x1b[201~', lambda: text('\x1b[2;2~'))
            probe('ctrl-v', b'\x1b[200~COPY_TEXT\x1b[201~', lambda: send('C-v'))
            command('clear; printf "\\033[31mPALETTE_MARKER\\033[0m\\n"')
            wait(lambda: 'PALETTE_MARKER' in screen(), 'ANSI output did not render')
            send('F6')
            colors = [f'#{0x112233 + i * 0x010101:06x}' for i in range(16)]
            command(":lua vim.api.nvim_set_hl(0, 'Normal', {bg='#233344',fg='#ddeeff'}); vim.rpcnotify(1, 'tuim_theme_changed', {bg_editor='#233344',bg_terminal='#233344',fg_primary='#ddeeff',terminal_colors={" + ','.join(repr(c) for c in colors) + "}})")
            expected_bg = (2, 0x23, 0x33, 0x44)

            def colors_match():
                lines = screen().splitlines()
                header = next((y for y, line in enumerate(lines) if '[Terminal]' in line), None)
                if header is None: return False
                row = next((y for y in range(header + 1, len(lines)) if 'PALETTE_MARKER' in lines[y]), None)
                if row is None: return False
                x = lines[row].index('PALETTE_MARKER')
                ansi = screen(True).splitlines()
                cells = ansi_cells(ansi[row])
                return cells[x] == ((2, 0x12, 0x23, 0x34), expected_bg) and ansi_cells(ansi[header + 2])[-1][1] == expected_bg

            wait(colors_match, 'Existing terminal output did not follow the editor theme')
            send('C-t', 'C-t')  # focus terminal, then hide it
            wait(lambda: '[Terminal]' not in screen(), 'Terminal did not hide')
            send('C-t')
            command('printf "%s:%s" "$$" "$TUIM_NATIVE_SESSION" > persisted')
            wait(lambda: marker('persisted'), 'Reopened shell did not accept input')
            assert (base / 'persisted').read_text() == f'{shell_pid}:preserved'
            command('exit')
            wait(lambda: 'Shell exited' in screen(), 'Exited shell did not show restart action')
            send('Enter')
            command("printf '%s' $$ > restarted.pid")
            wait(lambda: marker('restarted.pid'), 'Enter did not restart the exited shell')
            restarted = int((base / 'restarted.pid').read_text())
            assert restarted != shell_pid
            send('C-q')
            wait(lambda: subprocess.run(['ps', '-p', str(restarted)], capture_output=True).returncode != 0,
                 'Closing Tuim left the shell running')
        finally:
            subprocess.run(['tmux', '-S', socket, 'kill-server'], capture_output=True)
    print('Native terminal UI passed: direct shell parent, single Escape, control keys, bracketed paste, live theme/palette, persistent session, restart and cleanup')


if __name__ == '__main__':
    run()
