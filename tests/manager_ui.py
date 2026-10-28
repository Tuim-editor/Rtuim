#!/usr/bin/env python3
"""Exercise unified dialogs, search, scrolling, and settings persistence in tmux."""
import argparse
import json
import os
import pathlib
import shlex
import subprocess
import tempfile
import time

ROOT = pathlib.Path(__file__).resolve().parents[1]


def run(capture=None):
    with tempfile.TemporaryDirectory(prefix="tuim-manager-ui-") as directory:
        base = pathlib.Path(directory)
        data = base / "data/tuim"
        data.mkdir(parents=True)
        for name in ("config", "state", "cache"):
            (base / name).mkdir()
        settings_path = data / "settings.json"
        settings_path.write_text(json.dumps({"mode": "normal", "nerd_fonts": False, "clipboard": True}))
        (base / "sample.txt").write_text("Dialog test fixture\n")
        plugins = [
            {"name": f"plugin-{i:02}", "full_name": f"example/plugin-{i:02}",
             "description": "A useful extension with a longer description for testing layout and scrolling.",
             "tags": ["colorscheme"], "stars": {"curr": 1000 - i}}
            for i in range(40)
        ]
        (data / "store_db.json").write_text(json.dumps({"items": plugins}))
        (data / "user_plugins.json").write_text(json.dumps([p["full_name"] for p in plugins[:3]]))
        for p in plugins[:3]:
            (data / "lazy" / p["name"]).mkdir(parents=True)
        socket = str(base / "tmux.sock")

        def tmux(*args):
            return subprocess.check_output(["tmux", "-S", socket, *args], text=True)

        def screen(ansi=False):
            return tmux("capture-pane", "-p", *(["-e"] if ansi else []), "-t", "ui")

        def wait(predicate, description):
            deadline = time.monotonic() + 8
            while time.monotonic() < deadline:
                grid = screen()
                if predicate(grid):
                    return grid
                time.sleep(.05)
            raise AssertionError(description + "\n" + screen())

        def send(*keys):
            for key in keys:
                tmux("send-keys", "-t", "ui", key)
                time.sleep(.12)

        def text(value):
            tmux("send-keys", "-l", "-t", "ui", value)

        def click(label):
            grid = wait(lambda s: label in s, "Missing control: " + label)
            y, row = next((y, row) for y, row in enumerate(grid.splitlines()) if label in row)
            x = row.index(label)
            text(f"\x1b[<0;{x+1};{y+1}M\x1b[<0;{x+1};{y+1}m")
            time.sleep(.15)

        def locate(label):
            grid = wait(lambda s: label in s, "Missing control: " + label)
            y, row = next((y, row) for y, row in enumerate(grid.splitlines()) if label in row)
            return row.index(label), y

        def hover_padding(label):
            x, y = locate(label)
            text("\x1b[<35;1;1M")
            time.sleep(.15)
            before = screen(True).splitlines()[y]
            plain = screen()
            # Empty space inside the card must highlight before any click.
            text(f"\x1b[<35;{x + 30};{y + 1}M")
            wait(lambda s: screen(True).splitlines()[y] != before, "Card padding did not hover")
            assert screen() == plain, "Hover changed settings state"
            text("\x1b[<35;1;1M")
            wait(lambda s: screen(True).splitlines()[y] == before, "Hover did not clear")

        def command(query):
            send("F1")
            wait(lambda s: "Commands / type to filter" in s, "Commands did not open")
            text(query)
            send("Enter")

        def resize(width, height):
            tmux("resize-window", "-t", "ui", "-x", str(width), "-y", str(height))
            time.sleep(.3)

        def save_capture(name):
            if capture:
                capture.mkdir(parents=True, exist_ok=True)
                (capture / (name + ".txt")).write_text(screen())
                (capture / (name + ".ansi")).write_text(screen(True))

        env = {f"XDG_{name}_HOME": str(base / name.lower())
               for name in ("CONFIG", "DATA", "STATE", "CACHE")}
        env.update(TUIM_DISABLE_PLUGINS="1", TUIM_SKIP_ONBOARDING="1", TERM="xterm-256color")
        launch = shlex.join(["env", *(f"{k}={v}" for k, v in env.items()),
                             os.environ.get("TUIM_TEST_BINARY", str(ROOT / "target/debug/tuim")), str(base / "sample.txt")])
        try:
            tmux("new-session", "-d", "-s", "ui", "-x", "190", "-y", "55", "-c", str(base), launch)
            wait(lambda s: "EXPLORER" in s, "Workspace did not open")
            # Match the light-blue selection and dark panels in the reported screenshot.
            text(":lua vim.rpcnotify(1, 'tuim_theme_changed', {bg_sidebar='#141b24', bg_accent='#88aadd', fg_primary='#d4d4d4', fg_accent='#d4d4d4'})")
            send("Enter")
            command("extensions")
            wait(lambda s: "Your plugins" in s and "plugin-00" in s, "Extensions did not open")
            click("2 Discover")
            grid = wait(lambda s: "40 plugins" in s and "example/plugin-00" in s, "Discovery did not populate")
            assert grid.count("example/plugin-00") == 1, "Plugin is duplicated in sidebar and content"
            assert "All Categories" in grid and "[ Close ]" in grid
            send("End")
            wait(lambda s: "> plugin-39" in s, "End did not scroll to the final extension")
            save_capture("extensions-wide")
            resize(80, 24)
            wait(lambda s: "example/plugin-39" in s, "Selected extension disappeared after resize")
            save_capture("extensions-compact")
            send("Home", "Enter")
            wait(lambda s: "E  Configure plugin" in s, "Plugin details did not open")
            send("Escape")
            resize(190, 55)
            click("Colorschemes")
            send("Tab", "End")
            wait(lambda s: "> plugin-39" in s, "Category navigation did not return focus to results")
            send("/")
            text("\x1b[200~plugin-39\x1b[201~")
            send("Enter")
            grid = wait(lambda s: "1 plugins" in s, "Extension search did not filter results")
            assert "example/plugin-38" not in grid
            text("\x1b[200~PASTE-MUST-STAY-IN-DIALOG\x1b[201~")
            send("Escape")
            grid = wait(lambda s: "Dialog test fixture" in s and "[ Close ]" not in s, "Extensions did not close")
            assert "PASTE-MUST-STAY-IN-DIALOG" not in grid, "Modal paste leaked into the editor"
            command("settings")
            wait(lambda s: "Settings [User]" in s and "System Clipboard" in s, "Settings did not open")
            # Replace the extracted updater with a local fixture; never download or install in tests.
            updater = data / "software_update.sh"
            updater.write_text('printf "35" > "$3"\nsleep 1\nprintf "success" > "$1"\n')
            click("About")
            click("Update Tuim")
            wait(lambda s: "Updating Tuim" in s and "35%" in s, "Update progress did not render")
            click("Update Tuim")
            wait(lambda s: "Update complete. Restart Tuim" in s, "Update completion did not render")
            # Keyboard search and Enter must activate the same update control.
            updater.write_text('printf "fixture failure" > "$2"\nprintf "failure" > "$1"\n')
            send("/")
            text("Update Tuim")
            send("Enter", "Enter")
            wait(lambda s: "Update failed. Log:" in s, "Keyboard update failure did not render")
            send("Escape")
            command("settings")
            wait(lambda s: "System Clipboard" in s, "Settings did not reopen")
            hover_padding("System Clipboard")
            hover_padding("Mode:")
            # Escape and a mouse report can arrive together while moving the pointer.
            text("\x1b\x1b[<35;99;29M")
            wait(lambda s: "Settings [User]" not in s and "Dialog test fixture" in s,
                 "Escape followed by mouse motion did not return to the workspace")
            command("settings")
            field_x, field_y = locate("normal")
            click("Mode:")
            option_x, option_y = locate("Normal ")
            assert abs(option_x - field_x) < 16 and option_y == field_y + 2, "Mode dropdown is not anchored to its field"
            # Hovering an option must not commit it or move the popup.
            text(f"\x1b[<35;{option_x + 1};{option_y + 2}M")
            time.sleep(.15)
            assert "Unsaved" not in screen()
            assert locate("Normal ") == (option_x, option_y)
            text("\x1b\x1b[<35;99;29M")
            wait(lambda s: "│ Normal" not in s and "Settings [User]" in s,
                 "Escape did not dismiss only the dropdown")
            # Clicking a popup option uses the same geometry as rendering.
            click("Mode:")
            click("IDE ")
            wait(lambda s: "Unsaved" in s and "ide" in s, "Dropdown click did not select IDE")
            click("Revert")
            wait(lambda s: "· Unsaved" not in s and "normal" in s, "Revert did not restore mode")
            send("Escape")
            wait(lambda s: "Settings [User]" not in s, "Escape did not close reverted settings")
            command("settings")
            save_capture("settings-wide")
            click("System Clipboard")
            wait(lambda s: "Unsaved" in s, "Mouse did not edit a settings control")
            send("Escape")
            wait(lambda s: "Unsaved Settings" in s, "Cancel failed to protect unsaved settings")
            click("[N] Cancel")
            wait(lambda s: "Unsaved Settings" not in s, "Discard dialog did not cancel")
            click("Save Ctrl+S")
            wait(lambda s: "Settings [User]" not in s, "Save button did not close settings")
            assert json.loads(settings_path.read_text())["clipboard"] is False
            command("settings")
            send("/")
            text("\x1b[200~tab size\x1b[201~")
            send("Enter")
            wait(lambda s: "Tab Size:" in s and "System Clipboard" not in s, "Settings search did not navigate to Editor")
            previous_tabstop = json.loads(settings_path.read_text())["tabstop"]
            send("Enter")
            wait(lambda s: "Unsaved" in s, "Filtered settings row did not activate")
            send("C-s")
            assert json.loads(settings_path.read_text())["tabstop"] == {2: 4, 4: 8, 8: 2}[previous_tabstop]
            command("settings")
            click("Appearance")
            click("Theme:")
            send("End")
            send(*(["Down"] * 12))
            wait(lambda s: "Nightfox" in s, "Theme picker did not scroll to final option")
            send("Escape")
            resize(60, 20)
            wait(lambda s: "Theme:" in s and "[ Save Ctrl+S ]" in s, "Compact settings controls are clipped")
            save_capture("settings-compact")
            resize(35, 10)
            send("4", "Right", "Down")
            wait(lambda s: "Enable Lazy.nvim" in s, "Selected settings row disappeared in a small terminal")
            save_capture("settings-small")
            send("Escape")
            resize(190, 55)
            command("settings")
            click("Plugins")
            click("Mason Settings...")
            wait(lambda s: "Mason Package Manager" in s, "Mason did not open")
            save_capture("mason-wide")
            send("Escape")
            command("settings")
            click("Plugins")
            click("Plugin Manager...")
            wait(lambda s: "Plugin runtime" in s, "Plugin manager did not open")
            save_capture("plugin-manager-wide")
            send("Escape", "C-q")
        finally:
            subprocess.run(["tmux", "-S", socket, "kill-server"], capture_output=True)
    print("Unified manager UI passed: single plugin list, scrolling, search, settings save/cancel, dropdowns, resize, shared tool dialogs")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--capture", type=pathlib.Path)
    run(parser.parse_args().capture)
