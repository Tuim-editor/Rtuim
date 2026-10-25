#!/usr/bin/env python3
"""Regress Workspace [Esc] navigation and passive sidebar row hover."""
import json
import os
import pathlib
import shlex
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]


def run():
    with tempfile.TemporaryDirectory(prefix="tuim-sidebar-ui-") as directory:
        base = pathlib.Path(directory)
        for name in ("config", "data/tuim", "state", "cache"):
            (base / name).mkdir(parents=True)
        (base / "data/tuim/settings.json").write_text(json.dumps({"mode": "normal", "nerd_fonts": False}))
        sample = base / "a.txt"
        sample.write_text("sidebar regression fixture\n")
        (base / "b.txt").write_text("second file\n")
        socket = str(base / "tmux.sock")

        def tmux(*args):
            return subprocess.check_output(["tmux", "-S", socket, *args], text=True)

        def screen(ansi=False):
            return tmux("capture-pane", "-p", *(["-e"] if ansi else []), "-t", "ui")

        def wait(predicate, message):
            deadline = time.monotonic() + 6
            while time.monotonic() < deadline:
                if predicate():
                    return
                time.sleep(.05)
            raise AssertionError(message + "\n" + screen())

        def text(value):
            tmux("send-keys", "-l", "-t", "ui", value)

        def send(*keys):
            for key in keys:
                tmux("send-keys", "-t", "ui", key)
                time.sleep(.1)

        def locate(label):
            wait(lambda: label in screen(), "Missing " + label)
            return next((line.index(label), y) for y, line in enumerate(screen().splitlines())
                        if label in line and y > 0)

        def hover_row(label, x=20):
            _, y = locate(label)
            text("\x1b[<35;99;29M")
            time.sleep(.1)
            plain = screen()
            before = screen(True).splitlines()[y]
            text(f"\x1b[<35;{x + 1};{y + 1}M")
            wait(lambda: screen(True).splitlines()[y] != before, "No passive hover in row padding: " + label)
            assert screen() == plain, "Hover changed focus, selection, or text"
            # Moving within the same target must leave the same full-row highlight.
            text(f"\x1b[<35;2;{y + 1}M")
            time.sleep(.1)
            highlighted = screen(True).splitlines()[y]
            text(f"\x1b[<35;{x + 1};{y + 1}M")
            time.sleep(.1)
            assert screen(True).splitlines()[y] == highlighted, "Hover is only painting a single character"
            text("\x1b[<35;99;29M")
            wait(lambda: screen(True).splitlines()[y] == before, "Hover did not clear")

        def click(label):
            x, y = locate(label)
            text(f"\x1b[<0;{x + 1};{y + 1}M\x1b[<0;{x + 1};{y + 1}m")
            time.sleep(.15)

        def command(query):
            send("F1")
            wait(lambda: "Commands / type to filter" in screen(), "Palette did not open")
            text(query)
            send("Enter")

        def workspace():
            wait(lambda: "OPEN FILES" in screen() and "Workspace" in screen().splitlines()[-1],
                 "Escape did not return to the Workspace overview with sidebar focus")

        env = {f"XDG_{name}_HOME": str(base / name.lower()) for name in ("CONFIG", "DATA", "STATE", "CACHE")}
        env.update(TUIM_DISABLE_PLUGINS="1", TUIM_SKIP_ONBOARDING="1", TERM="xterm-256color")
        binary = os.environ.get("TUIM_TEST_BINARY", str(ROOT / "target/debug/tuim"))
        launch = shlex.join(["env", *(f"{key}={value}" for key, value in env.items()), binary, str(sample)])
        try:
            tmux("new-session", "-d", "-s", "ui", "-x", "100", "-y", "30", "-c", str(base), launch)
            wait(lambda: "EXPLORER" in screen(), "Explorer did not open")
            # No mouse press has been sent: the editor still owns keyboard focus.
            hover_row("b.txt")
            hover_row("< Workspace")
            send("F6", "Escape")
            workspace()
            hover_row("Explorer")
            hover_row("Settings")
            click("Explorer")
            wait(lambda: "EXPLORER" in screen(), "Explorer did not reopen")
            # First Escape cancels editing; the second navigates back.
            click("+F")
            send("Escape")
            assert "EXPLORER" in screen() and "New file:" not in screen()
            send("Escape")
            workspace()
            command("git")
            send("c")
            text("draft")
            send("Escape")
            assert "SOURCE CONTRO" in screen(), "Escape from commit input navigated too far"
            send("Escape")
            workspace()
            command("ai assistants")
            hover_row("Open chat")
            send("Escape")
            workspace()
            # A modal launched from Workspace must restore that same sidebar.
            click("Settings")
            wait(lambda: "Settings [User]" in screen(), "Settings did not open")
            send("Escape")
            workspace()
            click("Explorer")
            click("< Workspace")
            workspace()
            # Escape in the overview may return focus to the editor; it keeps the overview.
            send("Escape")
            wait(lambda: "Editor" in screen().splitlines()[-1], "Overview did not release editor focus")
            assert "OPEN FILES" in screen()
            assert sample.read_text() == "sidebar regression fixture\n"
            send("C-q")
        finally:
            subprocess.run(["tmux", "-S", socket, "kill-server"], capture_output=True)
    print("Sidebar UI passed: Escape returns to Workspace, nested cancellation, passive full-row hover before clicks, modal focus restoration")


if __name__ == "__main__":
    run()
