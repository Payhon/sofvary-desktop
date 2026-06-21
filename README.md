# Sofvary Desktop

Sofvary Desktop is the open desktop client for Sofvary, an AI-native local
software incubation platform.

Users describe intent in natural language. The desktop client manages local
workspaces, runtime packs, harness rules, generated app previews, and the
stealth shell UI that can be summoned when needed and hidden when the generated
app is running.

This public repository intentionally contains only the desktop client and
desktop-side development packages. Sofvary Cloud, the Sofvary website, the
Marketplace backend, validation services, deployment configuration, and private
operations code live outside this repository.

## Contents

```text
apps/
  desktop/              Tauri 2 desktop client
packages/
  i18n/                 Desktop-facing translations
  harness-compiler/     Prompt envelope and harness fixtures used by desktop tests
scripts/
  dev-desktop-safe.mjs
tests/
  e2e/
```

## Install

```bash
pnpm install
```

## Run

```bash
pnpm dev:desktop
```

## Validate

```bash
pnpm lint
pnpm typecheck
pnpm test
```

Rust checks:

```bash
cd apps/desktop/src-tauri
cargo fmt --check
cargo test -- --test-threads=1
cargo check
```

## Build Desktop

```bash
pnpm build:desktop
```

The GitHub Actions desktop build workflow creates unsigned beta artifacts for
Windows, macOS, and Linux. Production distribution still requires publisher
signing, notarization where applicable, updater signing, and platform-specific
installer validation.

## License

Source code in this repository is licensed under the Apache License, Version
2.0. The Sofvary name, logos, icons, and brand assets are not licensed under the
Apache License; see `TRADEMARKS.md`.
