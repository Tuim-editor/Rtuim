#!/usr/bin/env bash
set -euo pipefail
python3 - <<'CHECK'
from pathlib import Path
root = Path('docs/screenshots')
expected = {
    'explorer': 'EXPLORER', 'normal': 'WORKSPACE', 'terminal': 'tuim $',
    'settings': 'Settings [User]', 'source-control': 'SOURCE CONTROL',
    'extensions': 'Your plugins', 'ai': 'AI WORKSPACE',
    'worktrees': 'Worktree & Agent Orchestrator', 'ide-mode': 'IDE', 'zen': 'ZEN',
}
for name, marker in expected.items():
    data = (root / (name + '.webp')).read_bytes()
    assert data[:4] == b'RIFF' and data[8:12] == b'WEBP', name
    assert len(data) > 1000, name
    text = (root / (name + '.txt')).read_text()
    assert marker in text, (name, marker)
    assert 'Cargo.toml' in text, name
    for error in ('stack traceback', 'Press ENTER', 'Cannot make changes', 'build.zig'):
        assert error not in text, (name, error)
print('Ten Rust UI screenshots and terminal captures validated')
CHECK
