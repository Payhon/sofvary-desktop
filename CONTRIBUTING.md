# Contributing

Contributions to this repository are accepted under the Apache License,
Version 2.0.

Keep changes aligned with Sofvary Desktop's architecture:

- keep generated app UI separate from Sofvary shell UI
- keep OS-specific behavior behind the platform abstraction layer
- do not add raw shell command strings to runtime manifests
- keep generated apps inside their workspace boundary
- avoid dependencies that are not needed for the desktop client

Before opening a pull request, run:

```bash
pnpm install
pnpm lint
pnpm typecheck
pnpm test
```

For Rust changes, also run from `apps/desktop/src-tauri`:

```bash
cargo fmt --check
cargo test -- --test-threads=1
cargo check
```
