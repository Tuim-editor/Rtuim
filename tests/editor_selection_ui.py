#!/usr/bin/env python3
"""Check that Neovim selection highlights update before mouse release."""
import json
import os
import pathlib
import shlex
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
BINARY = os.environ.get("TUIM_TEST_BINARY", str(ROOT / "target/debug/tuim"))
FIXTURE = "selection must update while dragging across this line"


def run(mode):
    with tempfile.TemporaryDirectory(prefix="tuim-editor-selection-") as directory:
        base = pathlib.Path(directory)
        env = {"TUIM_DISABLE_PLUGINS": "1", "TUIM_SKIP_ONBOARDING": "1",
               "TERM": "xterm-256color", "SHELL": "/bin/sh"}
        for name in ("config", "data", "state", "cache"):
            (base / name).mkdir()
            env[f"XDG_{name.upper()}_HOME"] = str(base / name)
        (base / "data/tuim").mkdir()
        (base / "data/tuim/settings.json").write_text(json.dumps({
            "mode": mode, "nerd_fonts": False, "clipboard": False,
        }))
        sample = base / "sample.txt"
        sample.write_text(FIXTURE + "\n")
        socket = str(base / "tmux.sock")

        def tmux(*args):
            return subprocess.check_output(["tmux", "-S", socket, *args], text=True)

        def screen(ansi=False):
            return tmux("capture-pane", "-p", *(["-e"] if ansi else []), "-t", "ui")

        def text(value):
            tmux("send-keys", "-l", "-t", "ui", value)

        def wait(predicate, message):
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                if predicate():
                    return
                time.sleep(.04)
            raise AssertionError(f"{mode}: {message}\n{screen()}")

        launch = shlex.join(["env", *(f"{k}={v}" for k, v in env.items()), BINARY, str(sample)])
        try:
            tmux("new-session", "-d", "-s", "ui", "-x", "120", "-y", "36", "-c", str(base), launch)
            wait(lambda: FIXTURE in screen(), "Editor did not start")
            tmux("send-keys", "-t", "ui", "Escape")
            time.sleep(.15)
            row, line = next((y, line) for y, line in enumerate(screen().splitlines()) if FIXTURE in line)
            col = line.index(FIXTURE)

            def mouse(button, offset, suffix="M"):
                text(f"\x1b[<{button};{col + offset + 1};{row + 1}{suffix}")

            # ANSI captures include cell styling but not the terminal cursor.
            # The text stays fixed; only the selection highlight should change.
            before = screen(True).splitlines()[row]
            for button in (35, 39, 43, 51):
                mouse(button, 12)
                time.sleep(.1)
                assert screen(True).splitlines()[row] == before, "Passive hover changed editor styling"
            mouse(0, 0)
            time.sleep(.15)
            before = screen(True).splitlines()[row]
            mouse(32, 8)
            wait(lambda: screen(True).splitlines()[row] != before,
                 "Selection did not appear while holding the mouse button")
            selected = screen(True).splitlines()[row]
            mouse(32, 20)
            wait(lambda: screen(True).splitlines()[row] != selected,
                 "Selection did not extend while holding the mouse button")
            assert screen().splitlines()[row] == line, "Dragging changed buffer text"
            extended = screen(True).splitlines()[row]
            mouse(0, 20, "m")
            tmux("send-keys", "-t", "ui", "Escape")
            wait(lambda: screen(True).splitlines()[row] != extended,
                 "Selection highlight did not clear after Escape")
        finally:
            subprocess.run(["tmux", "-S", socket, "kill-server"], capture_output=True)


for mode in ("normal", "ide", "zen"):
    run(mode)
print("Editor selection passed: live drag highlighting and passive hover in Normal, IDE, and Zen modes")
