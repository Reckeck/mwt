# Contributing

Thanks for helping. Bug reports, translations and pull requests are welcome. For vulnerabilities use
[SECURITY.md](SECURITY.md), not a public issue.

## Setup

[mise](https://mise.jdx.dev) installs the pinned Node, pnpm and Rust (versions live in `mise.toml`).
You also need the [Tauri v2 prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS.

```bash
mise install
mise run setup        # pnpm install + cargo fetch
mise run dev          # Tauri dev; `pnpm dev` runs the UI on a mock backend (password: secret)
```

## Before you open a pull request

```bash
cargo fmt
mise run check        # lint, types, unit and static checks, tests, clippy, gen-types, platforms
```

`mise run check` must pass. Faster loops while working:

| Changed | Run |
|---|---|
| `src/**` | `mise run lint && mise run typecheck && mise run test-frontend` |
| `crates/**` | `mise run test && mise run clippy` |
| DTOs in `ipc_contract.rs` | `mise run gen-types` (never edit `ipc.gen.d.ts` by hand) |

Do not weaken or delete a check to make it pass. Architecture rules (layers, shared components, forms,
phone layout, onboarding for visible features) are in [AGENTS.md](AGENTS.md); the layout is described in
[docs/architecture.md](docs/architecture.md).

## Pull requests

- Branch from `development` and open the pull request into `development`. `main` is for releases.
- Keep one change per pull request; do not mix a refactor with a feature.
- Commit messages follow [Conventional Commits](https://www.conventionalcommits.org) in English, subject
  up to 72 characters: `feat`, `fix`, `docs`, `refactor`, `test`, `chore`, `perf`. The type decides the
  changelog section and the next version.
- Any version control tool works. The maintainer uses Jujutsu, but you do not have to.
- Vault format changes must stay compatible: add fields with `#[serde(default)]`, or bump the format
  version together with a migration ([docs/data-model.md](docs/data-model.md)).
- Money is always `Amount` (decimal), never `f64`.

## Translations

UI texts are in `src/shared/config/i18n/locales/ru.json` and `en.json`. Both files must have the same
keys. The documentation in `docs/` is being translated to English.
