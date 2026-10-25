# Performance profiling

Build the Rust executable before recording observations:

```bash
cargo build --locked --release
python3 scripts/profile_tuim.py --iterations 5 --warmup 1
python3 tests/performance_profile.py target/performance-profile.json
```

The harness writes `target/performance-profile.json`, which is ignored by Git.
It records the Rust compiler,
host target, binary SHA-256, optimization profile, sample distributions and
exit status. An uncommitted checkout is labeled `uncommitted`.

Scenarios cover startup, normal typing/navigation, Git refresh, 5,000 directory
entries, a 120,000-line file, resizing, terminal bursts, idle wakeups, and plugin
initialization when opted in with `TUIM_PROFILE_PLUGINS=1` and installed plugins
are available. Plugin profiling can require network access. Temporary fixtures and XDG
directories keep the run separate from user settings.

CI validates a small synthetic schema fixture in `tests/fixtures/`; it contains
no real performance measurements. Run `python3 tests/performance_profile.py`
without a path to check that fixture. Use the explicit generated report path
shown above to validate measurements from your own build.

Compare immutable binaries with `--baseline /path/to/old --candidate /path/to/new`
on equivalent hardware, Neovim/plugin state and terminal dimensions. “Settled”
means output was idle for the harness interval; it does not imply all language
servers finished starting. Plugin initialization is measured in headless Neovim.
