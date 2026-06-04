# CI Quality Gates

GitHub Actions runs `Quality Gates` on push, pull request, and manual dispatch.
The workflow is intentionally split so fast security failures stop later jobs:

- `security`: scans tracked files for obvious live secrets and blocks committed
  local artifacts, build outputs, wallets, mnemonics, env files, and the hidden
  local Codex skill directory.
- `rust`: runs `cargo fmt --all -- --check` and `cargo test --workspace`.
- `sdk`: runs `npm ci` and `npm run typecheck` from `sdk`.
- `integration-services`: starts Postgres and Redis service containers and checks
  readiness so future integration tests can attach to stable endpoints.
- `acton`: installs Acton on Linux and runs `acton build`, `acton test`,
  `acton check`, and `acton fmt --check`. This job is optional for now because
  Acton installation can vary by runner, but it still executes and reports
  failures.

Before committing locally, run the staged guards:

```powershell
python scripts/ci/secret_scan.py --staged
python scripts/ci/artifact_guard.py --staged
```

For Rust changes:

```powershell
cargo fmt --all -- --check
cargo test --workspace
```

For SDK changes:

```powershell
Set-Location sdk
npm ci
npm run typecheck
Set-Location ..
```

For Tolk contract changes, run Acton through WSL or Docker when native Windows
support is unavailable:

```powershell
acton build
acton test
acton check
acton fmt --check
```

The guard scripts inspect tracked files by default and staged files with
`--staged`. They deliberately allow placeholder values in `.env.example`, but
fail on live Redis/Postgres URLs, non-placeholder TON API/admin tokens, key or
mnemonic assignments, local database files, `target/`, `node_modules/`,
`sdk/dist/`, `.acton/`, `build/`, `gen/`, and local wallet/key material.
