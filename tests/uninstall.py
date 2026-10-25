#!/usr/bin/env python3
"""Selective removal must preserve unrelated Tuim and user files."""
import os
from pathlib import Path
import subprocess
import tempfile

ROOT = Path(__file__).resolve().parents[1]
files = {
    'binary': ['home/.local/bin/tuim'],
    'settings': ['config/tuim/init.lua', 'data/tuim/settings.json', 'data/tuim/settings.json.bak', 'data/tuim/settings.json.tmp'],
    'plugins': ['data/tuim/lazy/plugin/init.lua', 'data/tuim/mason/bin/server', 'data/tuim/site/parser/rust.so', 'data/tuim/plugin_inventory.json'],
    'cache': ['cache/tuim/cache'],
    'logs': ['state/tuim/log'],
    'sessions': ['state/tuim/sessions/session'],
    'other': ['data/tuim/runtime/tuim', 'data/tuim/user_plugins.json', 'data/tuim/plugin_states.json', 'data/tuim/plugin_configs/example.lua', 'state/tuim/shada/main.shada'],
    'unrelated': ['home/.local/bin/other', 'data/other/sentinel', 'state/other/sentinel'],
}
for flags in [['--' + name] for name in files if name not in ('other', 'unrelated')] + [['--plugins', '--settings'], ['--all'], []]:
    with tempfile.TemporaryDirectory(prefix="tuim uninstall ' ") as directory:
        base = Path(directory)
        env = os.environ.copy()
        env['HOME'] = str(base / 'home')
        for kind in ('config', 'data', 'state', 'cache'):
            env[f'XDG_{kind.upper()}_HOME'] = str(base / kind)
        for paths in files.values():
            for relative in paths:
                path = base / relative
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text('preserve me unless selected')
        result = subprocess.run(['bash', str(ROOT / 'uninstall.sh'), *flags, '--yes'], env=env, text=True, capture_output=True, check=True)
        full = not flags or '--all' in flags
        for group, paths in files.items():
            removed = (full and group != 'unrelated') or '--' + group in flags
            for relative in paths:
                assert (base / relative).exists() != removed, (flags, relative, result.stdout)
print('Uninstall passed: all selective flags, combined flags, full/default removal, custom XDG paths and preservation')
