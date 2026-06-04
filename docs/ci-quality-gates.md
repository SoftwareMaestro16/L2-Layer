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
- `acton`: installs Acton `1.1.0` on Linux, then runs the same contract check
  script used locally: `acton --version`, `acton doctor`, `acton build`,
  `acton test`, `acton check`, and `acton fmt --check`.

The Acton job is blocking. It uses
`ton-blockchain/setup-acton@2d38fd579e1bf8753a3e0cff9ad695612b98a676`
(`v1.0.0`) instead of `@master`, pins the Acton binary version to `1.1.0`,
and lets the setup action cache only the resolved binary. The official Acton CI
docs document checksum verification for downloaded archives and recommend
versioned Acton workflows. If the pinned action or Acton `1.1.0` release becomes
unavailable on `ubuntu-latest`, make the job optional again only with a linked
runner failure and a replacement pin.

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
wsl bash scripts/ci/acton_contract_checks.sh
```

On Linux or WSL with Acton already installed, the script uses the local `acton`
binary. If Acton is missing and Docker is available, it falls back to
`ghcr.io/ton-blockchain/acton:1.1.0`:

```powershell
wsl env ACTON_USE_DOCKER=1 bash scripts/ci/acton_contract_checks.sh
```

The Docker fallback mounts the repository at `/workspace`, sets `HOME` to
`/tmp/acton-home`, sets `XDG_CACHE_HOME` to `/tmp/acton-cache`, and passes
through only non-secret CI state such as `CI`, `GITHUB_ACTIONS`, and the lint
output-format override. Do not pass deployment mnemonics, wallet exports, TON API
keys, signer tokens, or `.env.local` into contract validation jobs. The script
does not run `acton wallet`, `acton script --net ...`, or any `--net mainnet`
command.

The guard scripts inspect tracked files by default and staged files with
`--staged`. They deliberately allow placeholder values in `.env.example`, but
fail on live Redis/Postgres URLs, non-placeholder TON API/admin tokens, key or
mnemonic assignments, local database files, `target/`, `node_modules/`,
`sdk/dist/`, `.acton/`, `build/`, `deployments/`, `gen/`, Acton wallet overlays,
deployment output JSON, and local wallet/key material.
