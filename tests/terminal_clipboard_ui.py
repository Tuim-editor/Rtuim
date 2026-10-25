#!/usr/bin/env python3
"""Exercise native terminal clipboard shortcuts through an isolated system provider."""
import json
import os
import pathlib
import shlex
import subprocess
import sys
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="tuim-clipboard-") as directory:
    base = pathlib.Path(directory)
    env = {"TUIM_DISABLE_PLUGINS": "1", "TUIM_SKIP_ONBOARDING": "1", "SHELL": "/bin/sh", "TERM": "xterm-256color"}
    for name in ("config", "data", "state", "cache", "bin"):
        (base / name).mkdir()
        if name != "bin": env[f"XDG_{name.upper()}_HOME"] = str(base / name)
    (base / "data/tuim").mkdir()
    (base / "data/tuim/settings.json").write_text(json.dumps({"mode": "normal", "nerd_fonts": False, "clipboard": True}))
    sample = base / "sample.txt"
    sample.write_text("editor fixture\n")
    clipboard = base / "clipboard"
    clipboard.write_text("")
    # Never access the user's real clipboard. Neovim detects these as its
    # system provider, so this also tests the actual clipboard RPC path.
    for executable, operation in (("wl-copy", "p.write_bytes(sys.stdin.buffer.read())"), ("wl-paste", "sys.stdout.buffer.write(p.read_bytes())")):
        script = base / "bin" / executable
        script.write_text(f"#!{sys.executable}\nimport pathlib, sys\np=pathlib.Path({str(clipboard)!r})\n{operation}\n")
        script.chmod(0o755)
    env.update(PATH=str(base / "bin") + ":" + os.environ["PATH"], WAYLAND_DISPLAY="tuim-test")
    (base / "probe.py").write_text("""import os, pathlib, select, sys, termios, tty
name, size = sys.argv[1], int(sys.argv[2])
saved = termios.tcgetattr(0)
try:
    tty.setraw(0)
    os.write(1, b'\\x1b[?2004hPASTE_READY\\r\\n')
    received = b''
    while len(received) < size:
        if not select.select([0], [], [], 5)[0]: break
        received += os.read(0, size - len(received))
    pathlib.Path(name).write_bytes(received)
finally:
    os.write(1, b'\\x1b[?2004l')
    termios.tcsetattr(0, termios.TCSANOW, saved)
""")
    socket = str(base / "tmux.sock")

    def tmux(*args):
        return subprocess.check_output(["tmux", "-S", socket, *args], text=True)

    def screen():
        return tmux("capture-pane", "-p", "-t", "ui")

    def wait(predicate, message):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if predicate(): return
            time.sleep(.04)
        raise AssertionError(message + "\n" + screen())

    def text(value):
        tmux("send-keys", "-l", "-t", "ui", value)

    def send(key):
        tmux("send-keys", "-t", "ui", key)
        time.sleep(.12)

    def command(value):
        text(value)
        send("Enter")

    def paste(name, expected_text, sequence):
        command("clear")
        wait(lambda: "PASTE_READY" not in screen(), "Old paste probe not cleared")
        expected = ("\x1b[200~" + expected_text + "\x1b[201~").encode()
        command(f"python3 probe.py {name} {len(expected)}")
        wait(lambda: "PASTE_READY" in screen(), "Paste probe did not start")
        text(sequence)
        wait(lambda: (base / name).exists(), "Paste shortcut did not deliver bytes")
        assert (base / name).read_bytes() == expected, (name, (base / name).read_bytes())

    binary = os.environ.get("TUIM_TEST_BINARY", str(ROOT / "target/debug/tuim"))
    launch = shlex.join(["env", *(f"{k}={v}" for k, v in env.items()), binary, str(sample)])
    try:
        tmux("new-session", "-d", "-s", "ui", "-x", "120", "-y", "36", "-c", str(base), launch)
        wait(lambda: "editor fixture" in screen(), "Editor did not start")
        send("C-t")
        for pane in ("bottom", "vertical-zen"):
            for shortcut, sequence in (("ctrl-v", "\x16"), ("shift-insert", "\x1b[2;2~"), ("ctrl-shift-v", "\x1b[118;6u"), ("encoded-ctrl-v", "\x1b[118;5u")):
                value = "external 界\nsecond line"
                clipboard.write_text(value)
                paste(pane + shortcut, value, sequence)
            for shortcut, sequence in (("ctrl-c", "\x03"), ("ctrl-insert", "\x1b[2;5~"), ("ctrl-shift-c", "\x1b[99;6u")):
                command("clear; printf 'COPY_MARKER\\n'")
                wait(lambda: "COPY_MARKER" in screen(), "Copy fixture did not render")
                row, line = next((y, line) for y, line in enumerate(screen().splitlines()) if "COPY_MARKER" in line)
                col = line.index("COPY_MARKER")
                text(f"\x1b[<0;{col+1};{row+1}M\x1b[<32;{col+11};{row+1}M\x1b[<0;{col+11};{row+1}m")
                clipboard.write_text("stale clipboard")
                text(sequence)
                wait(lambda: clipboard.read_text() == "COPY_MARKER", "Selection was not copied to the system clipboard")
                paste(pane + shortcut, "COPY_MARKER", "\x16")
            if pane == "bottom":
                send("F11")
                wait(lambda: "Return" in screen(), "Zen did not open")
                text(" oT")
                wait(lambda: "Vertical terminal" in screen().splitlines()[-1], "Vertical terminal did not open in Zen")
                paste("cross-pane", "COPY_MARKER", "\x16")
        send("F6")
        command(":call writefile([getreg('+')], 'editor.clipboard')")
        wait(lambda: (base / "editor.clipboard").exists(), "Editor clipboard read failed")
        assert (base / "editor.clipboard").read_text().strip() == "COPY_MARKER"
        send("C-q")
    finally:
        subprocess.run(["tmux", "-S", socket, "kill-server"], capture_output=True)
print("Terminal clipboard passed: Ctrl+C/V, Ctrl+Insert, Shift+Insert, Ctrl+Shift+C/V, system provider, multiline Unicode, cross-pane/editor sharing, Zen")
