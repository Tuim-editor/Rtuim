# Performance profiling

Build the Rust executable before recording observations:

```bash
cargo build --locked --release
python3 scripts/profile_tuim.py --iterations 5 --warmup 1
python3 tests/performance_profile.py
```

The harness writes `docs/performance-profile.json`. It records the Rust compiler,
host target, binary SHA-256, optimization profile, sample distributions and
exit status. An uncommitted checkout is labeled `uncommitted`.

Scenarios cover startup, normal typing/navigation, Git refresh, 5,000 directory
entries, a 120,000-line file, resizing, terminal bursts, idle wakeups, and plugin
initialization when opted in with `TUIM_PROFILE_PLUGINS=1` and installed plugins
are available. Plugin profiling can require network access. Temporary fixtures and XDG
directories keep the run separate from user settings.

The checked-in record is a local Rust release-build observation with two measured
iterations and no warmup. It replaces the obsolete Zig baseline. This small
sample is a smoke measurement, not evidence of cross-platform performance or
improvement over the former implementation. Consult the JSON for the host,
compiler, timestamp, sample values, and unavailable scenarios.

Compare immutable binaries with `--baseline /path/to/old --candidate /path/to/new`
on equivalent hardware, Neovim/plugin state and terminal dimensions. “Settled”
means output was idle for the harness interval; it does not imply all language
servers finished starting. Plugin initialization is measured in headless Neovim.
