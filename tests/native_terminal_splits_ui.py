#!/usr/bin/env python3
"""Leader terminal panes must be direct Tuim PTYs, also in Zen mode."""
import json
import os
import pathlib
import shlex
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]
with tempfile.TemporaryDirectory(prefix="tuim-native-splits-") as directory:
    base = pathlib.Path(directory)
    env = {"TUIM_DISABLE_PLUGINS": "1", "TUIM_SKIP_ONBOARDING": "1", "SHELL": "/bin/sh", "TERM": "xterm-256color"}
    for name in ("config", "data", "state", "cache"):
        (base / name).mkdir()
        env[f"XDG_{name.upper()}_HOME"] = str(base / name)
    (base / "data/tuim").mkdir()
    (base / "data/tuim/settings.json").write_text(json.dumps({"mode": "normal", "nerd_fonts": False, "clipboard": False}))
    sample = base / "sample.txt"
    sample.write_text("editor fixture\n")
    subprocess.run(["git", "init", "-q", str(base)], check=True)
    (base / ".gitignore").write_text("*\n!sample.txt\n")
    socket = str(base / "tmux.sock")

    def tmux(*args):
        return subprocess.check_output(["tmux", "-S", socket, *args], text=True)

    def screen():
        return tmux("capture-pane", "-p", "-t", "ui")

    def wait(predicate, message):
        deadline = time.monotonic() + 8
        while time.monotonic() < deadline:
            if predicate():
                return
            time.sleep(.04)
        raise AssertionError(message + "\n" + screen())

    def text(value):
        tmux("send-keys", "-l", "-t", "ui", value)

    def send(*keys):
        for key in keys:
            tmux("send-keys", "-t", "ui", key)
            time.sleep(.12)

    def command(value):
        text(value)
        send("Enter")

    def marker(name):
        return (base / name).exists() and (base / name).stat().st_size > 0

    def editor():
        # Both terminal regions have a direct path back to the editor with F6.
        for _ in range(4):
            if "Editor" in screen().splitlines()[-1]:
                return
            send("F6")
        raise AssertionError("Could not focus editor\n" + screen())

    def leader(key):
        editor()
        send("Escape")
        text(" o" + key)
        time.sleep(.25)

    def headers():
        return [(y, line.index("[Terminal]")) for y, line in enumerate(screen().splitlines()) if "[Terminal]" in line]

    def assert_parent(name, app_pid):
        wait(lambda: marker(name), "No shell PID for " + name)
        pid = int((base / name).read_text())
        parent = int(subprocess.check_output(["ps", "-p", str(pid), "-o", "ppid="], text=True))
        assert parent == app_pid, (name, "shell still belongs to Neovim", parent, app_pid)
        return pid

    binary = os.environ.get("TUIM_TEST_BINARY", str(ROOT / "target/debug/tuim"))
    launch = shlex.join(["env", *(f"{k}={v}" for k, v in env.items()), binary, str(sample)])
    try:
        tmux("new-session", "-d", "-s", "ui", "-x", "120", "-y", "36", "-c", str(base), launch)
        wait(lambda: "editor fixture" in screen(), "Editor did not start")
        app_pid = int(tmux("display-message", "-p", "-t", "ui", "#{pane_pid}"))
        send("F1")
        text("Git / Changes")
        send("Enter")
        wait(lambda: "SOURCE CONTROL" in screen() and "sample.txt" in screen(), "Git panel did not open")
        send("+")
        wait(lambda: subprocess.check_output(["git", "-C", str(base), "ls-files", "--cached"], text=True).strip() == "sample.txt", "Plus did not stage file")
        send("-")
        wait(lambda: not subprocess.check_output(["git", "-C", str(base), "ls-files", "--cached"], text=True).strip(), "Minus did not unstage on unborn branch")
        assert sample.read_text() == "editor fixture\n", "Unstage changed working file"
        send("F1")
        text("Project files")
        send("Enter")
        editor()
        leader("t")
        wait(lambda: len(headers()) == 1 and "Terminal" in screen().splitlines()[-1], "Space o t did not open the native drawer")
        command("export PANE=bottom; printf %s $$ > bottom.pid")
        bottom_pid = assert_parent("bottom.pid", app_pid)
        leader("T")
        wait(lambda: len(headers()) == 2 and "Vertical terminal" in screen().splitlines()[-1], "Space o T did not open the native vertical pane")
        command("export PANE=vertical; printf %s $$ > vertical.pid")
        vertical_pid = assert_parent("vertical.pid", app_pid)
        assert bottom_pid != vertical_pid

        # New global shortcuts work from both native PTYs and skip hidden regions.
        for key, focus in (("S-F6", "Terminal"), ("S-F6", "Workspace"), ("S-F6", "Editor"),
                           ("S-F6", "Vertical terminal"), ("F6", "Editor"), ("F6", "Workspace"),
                           ("F6", "Terminal"), ("F6", "Vertical terminal")):
            send(key)
            wait(lambda: focus in screen().splitlines()[-1], "Focus shortcut failed: " + key + " -> " + focus)
        original = sorted(headers())
        send("M-Left")
        wait(lambda: sorted(headers())[0][1] == original[0][1] - 2, "Alt+Left did not expand right terminal")
        send("M-Right")
        wait(lambda: sorted(headers()) == original, "Alt+Right did not restore right terminal")
        send("M-Up")
        wait(lambda: sorted(headers())[1][0] == original[1][0] - 1, "Alt+Up did not expand drawer")
        send("M-Down")
        wait(lambda: sorted(headers()) == original, "Alt+Down did not restore drawer")
        send("F12")
        wait(lambda: "Report" in screen() and "Summary" in screen(), "F12 did not open report from PTY")
        send("Escape")
        editor()
        send("C-b")
        assert "Editor" in screen().splitlines()[-1], "Hidden Ctrl+B sidebar alias remains"
        text(" e")
        wait(lambda: "Workspace" in screen().splitlines()[-1], "Space e did not focus native Explorer")
        original = sorted(headers())
        send("M-Right")
        wait(lambda: sorted(headers())[1][1] == original[1][1] + 2, "Alt+Right did not expand sidebar")
        send("M-Left")
        wait(lambda: sorted(headers()) == original, "Alt+Left did not restore sidebar")
        # Hiding and reopening each pane must preserve its own process and state.
        for key, name, pid in (("T", "vertical", vertical_pid), ("t", "bottom", bottom_pid)):
            leader(key)
            wait(lambda: len(headers()) == 1, "Leader shortcut did not hide " + name)
            leader(key)
            wait(lambda: len(headers()) == 2, "Leader shortcut did not reopen " + name)
            command(f'printf "%s:%s" "$$" "$PANE" > {name}.persisted')
            wait(lambda: marker(name + ".persisted"), "Reopened shell lost input")
            assert (base / (name + ".persisted")).read_text() == f"{pid}:{name}"

        send("F11")
        wait(lambda: not headers() and "Return" in screen(), "Zen did not hide the previous layout")
        leader("t")
        wait(lambda: len(headers()) == 1, "Bottom terminal unavailable in Zen")
        command('printf "%s:%s" "$$" "$PANE" > zen.bottom')
        wait(lambda: marker("zen.bottom"), "Zen bottom shell lost input")
        assert (base / "zen.bottom").read_text() == f"{bottom_pid}:bottom"
        leader("T")
        wait(lambda: len(headers()) == 2, "Vertical terminal unavailable in Zen")
        command('printf "%s:%s" "$$" "$PANE" > zen.vertical')
        wait(lambda: marker("zen.vertical"), "Zen vertical shell lost input")
        assert (base / "zen.vertical").read_text() == f"{vertical_pid}:vertical"

        original = sorted(headers())
        send("M-Left", "M-Up")
        wait(lambda: sorted(headers())[0][1] == original[0][1] - 2 and sorted(headers())[1][0] == original[1][0] - 1, "Zen keyboard resize failed")
        send("M-Right", "M-Down")
        wait(lambda: sorted(headers()) == original, "Zen keyboard resize did not restore panes")
        send("S-F6")
        wait(lambda: "Terminal" in screen().splitlines()[-1], "Zen reverse focus missed bottom terminal")
        send("S-F6")
        wait(lambda: "Editor" in screen().splitlines()[-1], "Zen reverse focus selected hidden sidebar")
        send("S-F6")
        wait(lambda: "Vertical terminal" in screen().splitlines()[-1], "Zen reverse focus failed to wrap")

        # Exact PTY dimensions after an idle window resize, with both panes in Zen.
        for width, height in ((90, 28), (150, 45), (120, 36)):
            tmux("resize-window", "-t", "ui", "-x", str(width), "-y", str(height))
            wait(lambda: len(screen().splitlines()) == height and len(headers()) == 2, "Zen resize lost panes")
            time.sleep(.2)
            command(f"stty size > size-{width}")
            wait(lambda: marker(f"size-{width}"), "Vertical PTY did not accept input after resize")
            top, bottom = sorted(headers())
            assert tuple(map(int, (base / f"size-{width}").read_text().split())) == (bottom[0] - top[0] - 1, width - (top[1] - 1))

        # Click the vertical divider and drag it; the live PTY must follow.
        top, _ = sorted(headers())
        divider = top[1] - 2
        text(f"\x1b[<0;{divider + 1};{top[0] + 3}M\x1b[<32;{divider - 9};{top[0] + 3}M\x1b[<0;{divider - 9};{top[0] + 3}m")
        wait(lambda: sorted(headers())[0][1] == top[1] - 10, "Vertical divider did not resize")
        command("stty size > dragged")
        wait(lambda: marker("dragged"), "Dragged PTY lost input")
        assert int((base / "dragged").read_text().split()[1]) == 50

        # Clicking close and using Ctrl+T still operate native panes in Zen.
        top, _ = sorted(headers())
        text(f"\x1b[<0;118;{top[0] + 1}M\x1b[<0;118;{top[0] + 1}m")
        wait(lambda: len(headers()) == 1, "Vertical close button failed")
        send("C-t", "C-t")
        wait(lambda: not headers(), "Ctrl+T did not hide the Zen drawer")
        send("C-t")
        wait(lambda: len(headers()) == 1, "Ctrl+T did not reopen the Zen drawer")
        send("F11")
        wait(lambda: len(headers()) == 2 and "Return" not in screen(), "Leaving Zen did not restore both panes")
        editor()
        # The existing command-menu action uses the same native vertical session.
        leader("T")
        wait(lambda: len(headers()) == 1, "Could not hide vertical pane before menu test")
        send("F1")
        text("Open terminal split right")
        send("Enter")
        wait(lambda: len(headers()) == 2 and "Vertical terminal" in screen().splitlines()[-1], "Menu did not open a native vertical pane")
        command('printf "%s:%s" "$$" "$PANE" > menu.vertical')
        wait(lambda: marker("menu.vertical"), "Menu terminal lost input")
        assert (base / "menu.vertical").read_text() == f"{vertical_pid}:vertical"
        editor()
        # Neither leader shortcut may create a Neovim terminal buffer.
        command(":lua local n=0; for _,b in ipairs(vim.api.nvim_list_bufs()) do if vim.bo[b].buftype=='terminal' then n=n+1 end end; vim.fn.writefile({tostring(n)}, 'nvim-terminals')")
        wait(lambda: marker("nvim-terminals"), "Could not inspect Neovim buffers")
        assert (base / "nvim-terminals").read_text().strip() == "0"
        # Resizing at minimum/compact geometries must remain responsive and reversible.
        for width, height in ((39, 12), (20, 8), (8, 4), (120, 36)):
            tmux("resize-window", "-t", "ui", "-x", str(width), "-y", str(height))
            time.sleep(.2)
            text("\x1b[1;3A\x1b[1;3B\x1b[1;3C\x1b[1;3D" * 12)
            time.sleep(.2)
            assert int(tmux("display-message", "-p", "-t", "ui", "#{pane_pid}")) == app_pid
        editor()
        send("F12")
        wait(lambda: "Summary" in screen(), "Compact resizing left UI unresponsive")
        send("Escape", "C-q")
        for pid in (bottom_pid, vertical_pid):
            wait(lambda: subprocess.run(["ps", "-p", str(pid)], capture_output=True).returncode != 0, "Quit leaked a shell")
    finally:
        subprocess.run(["tmux", "-S", socket, "kill-server"], capture_output=True)
print("Native terminal splits passed: both leader keys, direct PTYs, independent sessions, Zen, resize, focus, mouse close, no Neovim terminal buffers, cleanup")
