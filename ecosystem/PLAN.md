# Ecosystem Roadmap: Entropis L2 вокруг core L2

Этот файл описывает отдельную экосистему вокруг Entropis L2. Core L2 остается в `crates/l2-core`, `crates/l2-node` и `contracts/l1`; пользовательские приложения, SDK, обозреватели, кошельки, faucet, demo tools, deployment registry, docs portal, monitoring adapters и integration libraries должны жить отдельно в `ecosystem/`.

`ecosystem/PLAN.md` является tracked архитектурным roadmap-файлом. В отличие от локальных `STEP*.md`, этот файл можно коммитить в будущей задаче после проверок. В рамках создания этого файла не переносить текущие `sdk/` и `dashboard/`, не менять runtime-код, не выполнять commit и push.

## Целевая структура

- `ecosystem/sdk-js`: публичный TypeScript SDK, wallet helpers, TON Connect helpers, generated-wrapper facade.
- `ecosystem/explorer`: публичный explorer и operator dashboard, вынесенные из корневого `dashboard/`.
- `ecosystem/wallet`: lightweight web wallet или Telegram Mini App wallet flow.
- `ecosystem/faucet`: отдельный faucet app/service, который вызывает admin/node API и не живет внутри `l2-node`.
- `ecosystem/cli`: user/operator/demo CLI для account, faucet, transfer, deposit, withdraw, inspect.
- `ecosystem/registry`: public deployment registry, chain metadata, asset registry, public endpoint metadata.
- `ecosystem/docs`: user docs, integrator docs, public demo guides.
- `ecosystem/monitoring`: Prometheus/Grafana/OpenTelemetry adapters, dashboards and alert templates.
- `ecosystem/test-harness`: local-node integration harness and public-testnet smoke scenarios.

## Базовые правила

- `l2-node` предоставляет протокольный API, storage-backed status, auth, readiness and operator endpoints. UI, wallet, CLI and faucet UX не должны расти внутри `l2-node`.
- Текущий `sdk/` постепенно переносить или переэкспортировать через `ecosystem/sdk-js`.
- Текущий `dashboard/` постепенно переносить в `ecosystem/explorer`.
- `crates/l2-node/src/faucet.rs` оставить только как временный minimal internal grant primitive; публичный faucet вынести в `ecosystem/faucet`.
- Explorer/operator UI не расширять внутри `l2-node`; расширять только API endpoints, если ecosystem apps cannot function without them.
- Ecosystem не хранит private keys, mnemonics, `.env.local`, signer tokens, API keys, DB URLs, Redis URLs, raw signed BoCs.
- Для tracked изменений выполнять `cargo fmt --all -- --check`, `cargo test --workspace`, `npm ci`, `npm run typecheck`, Acton checks при Tolk changes, `python scripts/ci/secret_scan.py --staged`, `python scripts/ci/artifact_guard.py --staged`.
- Каждая будущая задача получает branch suggestion, Conventional Commit и push напрямую в GitHub без PR после успешных проверок, если пользователь не запретил push.

## 1. Ecosystem workspace and folder structure

### Цель

- Создать физическую структуру `ecosystem/` для всех приложений и библиотек вокруг Entropis.
- Разделить protocol core и user-facing ecosystem.
- Зафиксировать ownership boundaries, чтобы новые apps не появлялись случайно в `crates/l2-node`.

### Что вынести из L2

- Не переносить runtime core.
- Вынести roadmap ownership для `sdk/`, `dashboard/`, faucet UI, CLI, public docs and monitoring.
- Оставить в `l2-node` только API, auth, sequencer, relayer, indexer, storage, DA, observability primitives.

### Что реализовать в ecosystem

- Создать directories: `sdk-js`, `explorer`, `wallet`, `faucet`, `cli`, `registry`, `docs`, `monitoring`, `test-harness`.
- Добавить root `ecosystem/README.md` с назначением каждой папки.
- Добавить общий policy: no secrets, no local state, no generated build outputs.
- Описать dependency direction: ecosystem depends on public API/SDK contracts, core L2 never depends on ecosystem apps.

### Интерфейсы с l2-node

- Использовать только public REST/WS/operator API.
- Если endpoint нужен, сначала описать API contract, потом менять `l2-node`.
- Не читать Postgres/Redis напрямую из ecosystem apps.

### Пользовательский flow

- Developer видит `ecosystem/README.md`.
- Выбирает SDK, explorer, wallet, faucet or CLI.
- Запускает app against local or testnet L2 API without touching core node internals.

### Аудит и безопасность

- Проверить, что ecosystem folders не содержат secrets.
- Проверить, что `.env.local` examples only placeholders.
- Проверить, что apps do not bypass `l2-node` auth.
- Проверить, что admin-only flows clearly separated.

### Тесты и проверки

- `rg --files ecosystem`.
- Secret scan staged.
- Artifact guard staged.
- Runtime tests не нужны для folder-only step.

### Масштабируемость

- Structure должна поддерживать несколько packages/apps.
- Prepare future package manager workspace only after package boundaries are clear.
- Keep app-specific builds isolated.

### Рефакторинг и чистота

- Не переносить код в этой задаче.
- Не ломать existing `sdk/` and `dashboard/`.
- Document first, migrate later.

### Acceptance criteria

- `ecosystem/` exists with clear README/plan.
- Core L2 boundaries are explicit.
- No current app is moved.

### Git/GitHub

- Branch: `docs/ecosystem-workspace-plan`.
- Commit: `docs(ecosystem): define ecosystem workspace`.
- Push после docs/security checks.

## 2. Migration plan for current `sdk/`

### Цель

- Спланировать перенос current TypeScript SDK из `sdk/` в `ecosystem/sdk-js`.
- Сохранить совместимость для existing imports.
- Подготовить публичный SDK как отдельный package.

### Что вынести из L2

- `sdk/src` helpers, consensus vectors, generated wrapper facade, examples and README.
- Не выносить Rust consensus implementation.
- Не копировать generated wrappers вручную без Acton flow.

### Что реализовать в ecosystem

- `ecosystem/sdk-js/package.json`, `src`, `examples`, `tests`.
- Transitional re-export or compatibility package strategy for old `sdk/`.
- Package naming: `@entropis/sdk` or keep current name until release policy decides.
- Document semver, generated wrappers and vector tests.

### Интерфейсы с l2-node

- Use public API: submit tx, account, tx, block, withdrawal proof, explorer endpoints.
- Admin endpoints only through explicit operator client.
- No DB/Redis access.

### Пользовательский flow

- User installs SDK.
- Creates account, signs tx, submits to API.
- Builds TON Connect deposit/claim messages.
- Reads registry metadata.

### Аудит и безопасность

- Test keys only in examples.
- No private key persistence by default.
- Admin token never used in browser examples.
- Signature payload matches Rust consensus vectors.

### Тесты и проверки

- `npm ci`.
- `npm run typecheck`.
- Vector tests against Rust golden data.
- Example compile tests.
- Secret scan staged.

### Масштабируемость

- Keep SDK modular: client, signing, TON Connect, registry, generated wrappers.
- Prepare browser and Node builds.
- Avoid heavy dependencies in base client.

### Рефакторинг и чистота

- Migrate in small commits.
- Keep old `sdk/` as shim temporarily.
- Do not hand-edit generated files.

### Acceptance criteria

- Migration plan preserves existing SDK users.
- New package boundary is clear.
- Tests are mapped before moving code.

### Git/GitHub

- Branch: `refactor(ecosystem): plan sdk-js migration`.
- Commit: `docs(ecosystem): plan sdk migration`.
- Push после checks.

## 3. Migration plan for current `dashboard/`

### Цель

- Перенести static explorer/operator dashboard direction into `ecosystem/explorer`.
- Отделить UI от protocol node.
- Разделить public explorer and operator dashboard surfaces.

### Что вынести из L2

- Current `dashboard/index.html`, `dashboard/app.js`, `dashboard/styles.css`, `dashboard/README.md`.
- UI-specific code must not live in `crates/l2-node`.
- Keep `l2-node` explorer APIs as backend contracts only.

### Что реализовать в ecosystem

- `ecosystem/explorer` app plan with public and operator modes.
- Build/runtime choice: static app first, framework later only if needed.
- Registry loading and API base URL config.
- Public docs for running explorer locally.

### Интерфейсы с l2-node

- Public endpoints: summary, blocks, deposits, tx, account, withdrawal status.
- Operator endpoints: metrics, failures, relayer/finalizer status with auth.
- API errors must be safe and stable.

### Пользовательский flow

- User opens explorer.
- Enters or uses default API URL.
- Views blocks, txs, deposits, withdrawals and L1 contract links.
- Operator optionally enters admin token in memory only.

### Аудит и безопасность

- Admin token never stored in localStorage/sessionStorage.
- Public explorer cannot trigger admin actions.
- XSS check for hashes, reasons, addresses.
- CORS assumptions documented.

### Тесты и проверки

- Static smoke test.
- Browser/API mock test if framework added.
- Typecheck if JS/TS build exists.
- Secret scan.

### Масштабируемость

- Add pagination before large datasets.
- Keep explorer read-only by default.
- Prepare hosted deployment without core node changes.

### Рефакторинг и чистота

- Migrate files without changing behavior first.
- Keep UI state separate from API client.
- Use registry instead of hardcoded addresses.

### Acceptance criteria

- Dashboard migration path is documented.
- Public and operator concerns are separated.
- Existing dashboard can keep working during migration.

### Git/GitHub

- Branch: `refactor(ecosystem): plan explorer migration`.
- Commit: `docs(ecosystem): plan dashboard migration`.
- Push после checks.

## 4. Public Entropis JS SDK package

### Цель

- Сделать production-quality public JS/TS SDK для Entropis L2.
- Закрыть wallet, signing, deposits, withdrawals, registry, explorer read helpers.
- Дать third-party developers stable integration layer.

### Что вынести из L2

- Public client abstractions from current SDK.
- TON Connect payload builders.
- Registry loaders and generated wrapper facades.
- Demo helpers that do not belong in node.

### Что реализовать в ecosystem

- Typed API client.
- Signing helpers for L2 transactions.
- Deposit helpers for TON and Jetton.
- Withdrawal proof and claim helpers.
- Registry and asset metadata helpers.
- Browser-safe and Node-safe entrypoints.

### Интерфейсы с l2-node

- REST endpoints and WebSocket stream.
- Stable DTOs generated or manually typed.
- Operator methods require explicit admin client.

### Пользовательский flow

- `createClient({ apiUrl, registry })`.
- Generate or import test key.
- Request faucet via operator flow or receive deposit.
- Sign transfer/withdraw.
- Build TON Connect deposit/claim messages.

### Аудит и безопасность

- Never store private key automatically.
- Warn test-only key generation.
- Validate hash/address formats.
- Prevent accidental mainnet use until supported.

### Тесты и проверки

- Typecheck.
- Unit tests for payload builders.
- Consensus vector tests.
- Mock API tests.
- Browser bundle smoke if bundled.

### Масштабируемость

- Split modules to avoid large browser bundles.
- Support future wallet adapters.
- Support versioned API.

### Рефакторинг и чистота

- Keep generated wrappers isolated.
- Avoid duplicate encoders.
- Document public API.

### Acceptance criteria

- SDK covers core public demo flows.
- Typecheck and vector tests pass.
- API is documented.

### Git/GitHub

- Branch: `feat/ecosystem-sdk-js`.
- Commit: `feat(ecosystem): add public js sdk package`.
- Push после checks.

## 5. Wallet connection and TON Connect deposit helpers

### Цель

- Реализовать wallet-facing flow для TON Connect deposits and withdrawal claims.
- Подготовить web wallet and Telegram Mini App integration.
- Убрать ручное создание BoC payloads пользователем.

### Что вынести из L2

- UI/wallet payload logic out of `l2-node`.
- TON Connect app code out of core L2.
- Keep only protocol payload definitions shared through SDK.

### Что реализовать в ecosystem

- `ecosystem/wallet` proof-of-concept.
- TON Connect transaction builders.
- Deposit TON flow.
- Deposit Jetton flow through TEP-74 transfer.
- Claim withdrawal flow.
- Account and balance display.

### Интерфейсы с l2-node

- Account lookup.
- Withdrawal proof lookup.
- Registry lookup.
- Submit L2 tx through API.

### Пользовательский flow

- Connect TON wallet.
- Create L2 account.
- Deposit TON or Jetton to L2.
- See L2 balance.
- Withdraw and claim through wallet.

### Аудит и безопасность

- Validate destination vault/root addresses from registry.
- Prevent arbitrary payload signing.
- No wallet seed handling in app.
- Clear testnet-only warning.

### Тесты и проверки

- SDK payload tests.
- UI smoke with mocked wallet.
- Typecheck/build.
- Manual TON Connect testnet transaction.

### Масштабируемость

- Abstract wallet adapter.
- Prepare Telegram Mini App.
- Support multiple assets through registry.

### Рефакторинг и чистота

- Keep wallet UI separate from SDK core.
- Reuse SDK builders.
- Avoid duplicating TEP-74 encoding.

### Acceptance criteria

- Wallet can create valid deposit and claim messages.
- No private key handling.
- Testnet registry drives addresses.

### Git/GitHub

- Branch: `feat(ecosystem): wallet-ton-connect`.
- Commit: `feat(ecosystem): add ton connect wallet flows`.
- Push после checks.

## 6. Separate public faucet app/service

### Цель

- Вынести public faucet UX/service из `l2-node`.
- Оставить node faucet primitive minimal and admin-protected.
- Добавить rate limits, abuse controls and public request flow outside core.

### Что вынести из L2

- Browser/public faucet UI.
- Public rate limiting and captcha/allowlist logic.
- Faucet operator dashboard.
- Leave `EntFaucetService` as internal grant primitive until replaced by dedicated service contract/API.

### Что реализовать в ecosystem

- `ecosystem/faucet` app/service.
- Public request endpoint.
- Admin backend client to call `POST /v1/admin/faucet/ent`.
- Abuse controls: IP/account cooldown, captcha option, daily caps.
- Public status page.

### Интерфейсы с l2-node

- Admin faucet endpoint with bearer token.
- Account lookup.
- Mempool/block status for grant confirmation.
- Operator metrics if needed.

### Пользовательский flow

- User enters L2 account id.
- Faucet validates and grants ENT.
- User sees grant status and tx/block proof.
- Duplicate request returns existing state.

### Аудит и безопасность

- Admin token stays server-side only.
- Public service never exposes bearer token.
- Rate limit by IP/account.
- Validate account hash format.
- Do not allow arbitrary deposit events.

### Тесты и проверки

- Unit tests for rate limit.
- Mock node API tests.
- Abuse/duplicate tests.
- Secret scan.
- Typecheck/build.

### Масштабируемость

- Storage-backed rate limits.
- Queue grants if node unavailable.
- Metrics for abuse and grants.

### Рефакторинг и чистота

- Do not add captcha/rate-limit logic to `l2-node`.
- Keep node API simple.
- Document transition from admin faucet to public faucet service.

### Acceptance criteria

- Public faucet runs outside `l2-node`.
- Admin token not exposed.
- Duplicate and abuse paths handled.

### Git/GitHub

- Branch: `feat(ecosystem): public-faucet`.
- Commit: `feat(ecosystem): add public ent faucet service`.
- Push после checks.

## 7. Explorer app for blocks, txs, deposits, withdrawals, commitments

### Цель

- Создать public explorer app в `ecosystem/explorer`.
- Показывать L2 blocks, transactions, deposits, withdrawals, batch commitments and finalizations.
- Использовать только public endpoints and registry.

### Что вынести из L2

- Public explorer UI from root `dashboard/`.
- Formatting, navigation, filtering, charts.
- L1 Tonviewer/Toncenter links.

### Что реализовать в ecosystem

- Explorer home summary.
- Blocks list/detail.
- Tx lookup/detail.
- Deposit lookup/detail.
- Withdrawal lookup/detail.
- Batch commitment/finality views.
- Registry-based L1 links.

### Интерфейсы с l2-node

- `/v1/explorer/*`.
- `/v1/block/:height`.
- `/v1/tx/:hash`.
- `/v1/account/:id`.
- Public read-only only.

### Пользовательский flow

- User opens explorer.
- Searches tx/deposit/withdrawal hash.
- Opens block and sees roots/status.
- Follows L1 commitment links.

### Аудит и безопасность

- No admin token in public app.
- Escape all user-provided values.
- Bound query input.
- Safe error messages.

### Тесты и проверки

- UI smoke.
- API mock tests.
- Pagination tests.
- Typecheck/build.

### Масштабируемость

- Cursor pagination.
- Avoid large client-side scans.
- Prepare indexed explorer API later.

### Рефакторинг и чистота

- Keep API client reusable.
- Separate public explorer from operator dashboard.
- Avoid one giant app file.

### Acceptance criteria

- Explorer covers core public objects.
- It works against local node and testnet node.
- No admin capabilities exposed.

### Git/GitHub

- Branch: `feat(ecosystem): public-explorer`.
- Commit: `feat(ecosystem): add public explorer app`.
- Push после checks.

## 8. Operator dashboard separated from public explorer

### Цель

- Создать operator dashboard отдельно от public explorer.
- Показывать readiness, metrics, relayer/finalizer/indexer status and failures.
- Сохранить admin auth strictly server/API-side.

### Что вынести из L2

- Operator UI and diagnostics from root dashboard.
- Failure triage UX.
- Runbook links and alert presentation.

### Что реализовать в ecosystem

- Operator mode or separate app.
- Admin token input stored only in memory.
- Worker status panels.
- Failure lists with safe reason codes.
- Runbook deep links.

### Интерфейсы с l2-node

- `/readyz`.
- `/v1/operator/metrics`.
- `/v1/operator/failures`.
- `/v1/operator/batch-relayer`.
- `/v1/operator/batch-finalizer`.
- `/v1/mempool/metrics`.

### Пользовательский flow

- Operator enters API URL and token.
- Dashboard shows health and failing components.
- Operator follows runbook.
- No write actions until explicitly planned.

### Аудит и безопасность

- Token never persisted.
- Operator routes not accessible from public mode without token.
- Avoid rendering raw provider errors.
- No secret-bearing diagnostics.

### Тесты и проверки

- Auth behavior smoke.
- Redaction tests if code handles responses.
- UI typecheck/build.
- Manual local operator smoke.

### Масштабируемость

- Add polling with backoff.
- Prepare multi-node view later.
- Export dashboard JSON config.

### Рефакторинг и чистота

- Keep operator components separate from public explorer.
- Do not add business logic to UI.
- Use typed API client.

### Acceptance criteria

- Operator can diagnose node health.
- Public users cannot access operator data without token.
- No secrets stored in browser storage.

### Git/GitHub

- Branch: `feat(ecosystem): operator-dashboard`.
- Commit: `feat(ecosystem): split operator dashboard`.
- Push после checks.

## 9. User CLI for account, faucet, transfer, deposit and withdraw

### Цель

- Создать CLI для demo and developer workflows.
- Упростить account creation, faucet, transfer, deposit payload, withdrawal proof and claim.
- Дать scriptable alternative to UI.

### Что вынести из L2

- Demo scripts from root SDK.
- Manual curl examples.
- User-facing command orchestration.

### Что реализовать в ecosystem

- `ecosystem/cli`.
- Commands: `account new`, `account show`, `faucet request`, `tx transfer`, `deposit ton`, `deposit jetton`, `withdraw create`, `withdraw proof`, `withdraw claim`.
- Config from env or registry.
- Safe output JSON option.

### Интерфейсы с l2-node

- Public REST APIs.
- Admin faucet only through explicit operator mode.
- Registry metadata.

### Пользовательский flow

- Developer runs CLI against local/testnet.
- Performs full happy path without editing raw JSON.
- Gets hashes and links for explorer.

### Аудит и безопасность

- Do not write private keys by default.
- If key file support added, use ignored path and warnings.
- Never print admin token.
- Refuse mainnet by default.

### Тесты и проверки

- CLI unit tests.
- Mock API tests.
- Typecheck/build.
- Manual local smoke.

### Масштабируемость

- Plugin-like command structure.
- Support multiple profiles.
- Keep reusable SDK under `sdk-js`.

### Рефакторинг и чистота

- CLI uses SDK, not duplicate encoders.
- Keep commands small.
- Document env vars.

### Acceptance criteria

- CLI can run core demo flow.
- No raw JSON required.
- Safety warnings are explicit.

### Git/GitHub

- Branch: `feat(ecosystem): user-cli`.
- Commit: `feat(ecosystem): add entropis cli`.
- Push после checks.

## 10. Public deployment and asset registry

### Цель

- Создать public registry for chain, contracts, endpoints and assets.
- Стандартизировать metadata для SDK, explorer, wallet, faucet and docs.
- Убрать hardcoded addresses из apps.

### Что вынести из L2

- Public deployment metadata from docs/env notes.
- Asset presentation metadata from apps.
- Registry loading logic from individual tools.

### Что реализовать в ecosystem

- `ecosystem/registry/testnet/entropis.json`.
- Chain metadata.
- L1 contract addresses.
- Asset ids, symbols, decimals, logos.
- Public API endpoints.
- Registry schema and validator.

### Интерфейсы с l2-node

- Node can expose its chain id and maybe registry version.
- Apps compare registry chain id with API responses.
- No secrets in registry.

### Пользовательский flow

- SDK/explorer/wallet load registry.
- User sees assets and contract links.
- Operators update registry after redeploy.

### Аудит и безопасность

- Registry cannot include private endpoints or tokens.
- Validate TON testnet addresses.
- Mark deprecated deployments.
- Prevent accidental mainnet metadata until supported.

### Тесты и проверки

- Schema validation.
- Secret scan.
- Apps load registry in tests.
- Link validation manually.

### Масштабируемость

- Multiple environments.
- Multiple assets.
- Versioned deployments.

### Рефакторинг и чистота

- Single source of truth for public metadata.
- No duplicated addresses.
- Keep schema stable.

### Acceptance criteria

- Registry is valid and secret-free.
- SDK/explorer can consume it.
- Redeploy process is documented.

### Git/GitHub

- Branch: `feat(ecosystem): public-registry`.
- Commit: `feat(ecosystem): add public registry`.
- Push после checks.

## 11. Docs portal for developers and users

### Цель

- Вынести user/integrator docs into `ecosystem/docs`.
- Разделить protocol docs and ecosystem usage docs.
- Сделать public onboarding for developers and users.

### Что вынести из L2

- SDK usage docs.
- Dashboard usage docs.
- Wallet/faucet/CLI demo guides.
- Public testnet user guides.

### Что реализовать в ecosystem

- Docs portal structure.
- Quickstart.
- Wallet guide.
- SDK guide.
- Faucet guide.
- Explorer guide.
- Troubleshooting.

### Интерфейсы с l2-node

- Docs reference public API and registry.
- Link operator-only docs separately.
- Avoid private env examples.

### Пользовательский flow

- User follows quickstart.
- Gets ENT, deposits TON, transfers, withdraws.
- Developer integrates SDK.

### Аудит и безопасность

- No secrets in docs.
- Testnet-only limitations clear.
- Admin actions separated.
- No mainnet readiness claims.

### Тесты и проверки

- Link review.
- Secret scan.
- Example command smoke where possible.
- Markdown lint if added.

### Масштабируемость

- Docs can become static site later.
- Version docs by testnet deployment.
- Separate user/dev/operator personas.

### Рефакторинг и чистота

- Avoid duplicating protocol docs.
- Link to source docs.
- Keep examples current with SDK.

### Acceptance criteria

- New user can follow docs.
- Developer can integrate SDK.
- Operator-only data is not public.

### Git/GitHub

- Branch: `docs(ecosystem): portal`.
- Commit: `docs(ecosystem): add user docs portal`.
- Push после checks.

## 12. Demo scripts for live testnet prototype

### Цель

- Создать repeatable demo scripts for live testnet.
- Покрыть end-to-end path without manual API calls.
- Использовать SDK/CLI/registry.

### Что вынести из L2

- Manual demo steps from docs.
- SDK example scripts into ecosystem demo area.
- No core node orchestration inside demo scripts beyond API calls.

### Что реализовать в ecosystem

- `ecosystem/cli` or `ecosystem/test-harness/demo`.
- Scripts: setup account, faucet, transfer, deposit payload, wait block, commit status, withdraw.
- Output safe summary.
- Failure instructions.

### Интерфейсы с l2-node

- Public and operator APIs as needed.
- Admin token only for explicit operator demo.
- Registry addresses.

### Пользовательский flow

- Operator starts node.
- Runs demo script.
- Shares safe output with tx hashes and explorer links.

### Аудит и безопасность

- Do not log tokens.
- Do not store private keys unless explicit ignored path.
- Refuse mainnet.
- Bound waits and retries.

### Тесты и проверки

- Mock API tests.
- Local node smoke.
- Typecheck/build.
- Secret scan.

### Масштабируемость

- Scripts reusable in CI smoke.
- Config profiles for local/testnet.
- Modular steps.

### Рефакторинг и чистота

- Use CLI/SDK primitives.
- Avoid duplicated HTTP clients.
- Keep output stable.

### Acceptance criteria

- Demo path runs from one documented command sequence.
- Output is safe to share.
- Failures point to runbooks.

### Git/GitHub

- Branch: `feat(ecosystem): testnet-demo-scripts`.
- Commit: `feat(ecosystem): add live demo scripts`.
- Push после checks.

## 13. Monitoring dashboards and alert templates

### Цель

- Вынести monitoring adapters and dashboards into `ecosystem/monitoring`.
- Сделать operator observability reusable outside node source.
- Подготовить Prometheus/Grafana/OpenTelemetry assets.

### Что вынести из L2

- Dashboard templates.
- Alert rules.
- Metrics mapping docs.
- Runbook-linked alert descriptions.

### Что реализовать в ecosystem

- Grafana dashboard JSON.
- Prometheus alert templates.
- OpenTelemetry mapping plan.
- Metrics reference.
- Example local monitoring stack.

### Интерфейсы с l2-node

- `/v1/operator/metrics`.
- `/readyz`.
- Future Prometheus endpoint if added.
- Log fields and reason codes.

### Пользовательский flow

- Operator imports dashboard.
- Configures API/metrics source.
- Gets alerts for relayer, finalizer, mempool, indexer, DA.

### Аудит и безопасность

- Dashboards do not include tokens.
- Alert examples use placeholders.
- Public dashboards do not expose admin endpoints.
- Logs do not leak secrets.

### Тесты и проверки

- JSON validation.
- Secret scan.
- Manual import test.
- Metrics endpoint smoke.

### Масштабируемость

- Multi-node support later.
- Environment labels.
- Retention guidance.

### Рефакторинг и чистота

- Keep monitoring assets separate from Rust code.
- Avoid duplicating runbooks.
- Version dashboards with API.

### Acceptance criteria

- Monitoring assets exist and validate.
- Alerts map to runbook actions.
- No secrets in templates.

### Git/GitHub

- Branch: `feat(ecosystem): monitoring-assets`.
- Commit: `feat(ecosystem): add monitoring dashboards`.
- Push после checks.

## 14. Integration test harness against local node

### Цель

- Создать ecosystem test harness для black-box testing against local `l2-node`.
- Проверять public SDK/API flows without internal module access.
- Подготовить CI/manual smoke tests.

### Что вынести из L2

- End-to-end scripts that do not belong in unit tests.
- User-facing flow validation.
- Cross-app integration tests.

### Что реализовать в ecosystem

- `ecosystem/test-harness`.
- Test runner config.
- Local node preflight.
- Tests for faucet, transfer, explorer, proof, registry.
- Optional Postgres/Redis docker compose later.

### Интерфейсы с l2-node

- Public API and admin test token for local mode.
- No direct Rust imports.
- No DB/Redis access.

### Пользовательский flow

- Developer starts local node.
- Runs harness.
- Receives pass/fail report.

### Аудит и безопасность

- Test admin token from env only.
- No real secrets committed.
- Test accounts are throwaway.
- Refuse non-local destructive actions by default.

### Тесты и проверки

- Harness self-test.
- Local smoke.
- Typecheck/build.
- Secret scan.

### Масштабируемость

- Add testnet smoke profile.
- Parallel tests where independent.
- Reusable fixtures.

### Рефакторинг и чистота

- Keep black-box tests separate from Rust unit tests.
- Use SDK for API calls.
- Keep fixtures small.

### Acceptance criteria

- Harness validates basic local flows.
- It does not need internal node imports.
- It can run manually before release.

### Git/GitHub

- Branch: `test(ecosystem): local-harness`.
- Commit: `test(ecosystem): add local integration harness`.
- Push после checks.

## 15. Security review of ecosystem apps

### Цель

- Провести отдельный security review для ecosystem apps.
- Проверить browser, CLI, faucet, wallet, explorer and docs risks.
- Не смешивать app security with core protocol audit.

### Что вынести из L2

- App-layer threat model.
- Browser token storage rules.
- Faucet abuse model.
- SDK key-handling rules.

### Что реализовать в ecosystem

- `ecosystem/docs/security.md`.
- App threat matrix.
- Required checks per package.
- Release checklist.
- Incident notes for public apps.

### Интерфейсы с l2-node

- Auth boundaries.
- Public/admin API separation.
- Safe error handling.
- Rate limiting expectations.

### Пользовательский flow

- Developer checks security doc before app release.
- Operator reviews faucet/dashboard config.
- User-facing apps show testnet warnings.

### Аудит и безопасность

- XSS.
- Token leakage.
- Private key storage.
- Admin endpoint exposure.
- Faucet abuse.
- Supply-chain dependencies.
- Mainnet confusion.

### Тесты и проверки

- Secret scan.
- Dependency audit where available.
- UI security smoke.
- Auth tests.
- Manual review checklist.

### Масштабируемость

- Package-level security owners.
- Automated checks in CI.
- Release gating per app.

### Рефакторинг и чистота

- Keep security docs actionable.
- Link to core audit docs.
- Avoid false production claims.

### Acceptance criteria

- Ecosystem security checklist exists.
- Each app has required checks.
- No critical app risk ignored for public testnet.

### Git/GitHub

- Branch: `docs(ecosystem): security-review`.
- Commit: `docs(ecosystem): add app security checklist`.
- Push после checks.

## 16. Packaging, versioning and release process

### Цель

- Определить release process для ecosystem packages.
- Разделить protocol version, SDK version, app version, registry version.
- Подготовить npm/static app release later.

### Что вынести из L2

- SDK package release policy.
- App build/release policy.
- Registry versioning.
- Changelog rules.

### Что реализовать в ecosystem

- Versioning doc.
- Package naming strategy.
- Release checklist.
- Compatibility matrix.
- Deprecation policy for old endpoints/registries.

### Интерфейсы с l2-node

- API compatibility version.
- Chain id and registry version.
- Generated wrapper version.

### Пользовательский flow

- Developer installs SDK version matching registry/API.
- Operator deploys compatible explorer/faucet.
- Release notes explain changes.

### Аудит и безопасность

- Prevent publishing secrets in packages.
- Check npm files list.
- Verify source maps do not include secrets.
- Review dependency changes.

### Тесты и проверки

- Package dry-run.
- Typecheck/build.
- Secret scan.
- Artifact guard.
- Compatibility tests.

### Масштабируемость

- Support multiple packages.
- Automate changelog later.
- Keep semver discipline.

### Рефакторинг и чистота

- Do not release generated junk.
- Keep package boundaries clear.
- Document breaking changes.

### Acceptance criteria

- Release policy exists.
- Compatibility matrix exists.
- Package dry-run is planned.

### Git/GitHub

- Branch: `docs(ecosystem): release-process`.
- Commit: `docs(ecosystem): define release process`.
- Push после checks.

## 17. Future Telegram Mini App wallet

### Цель

- Спланировать Telegram Mini App wallet for Entropis.
- Использовать TON Connect and ecosystem SDK.
- Не блокировать current web wallet and CLI.

### Что вынести из L2

- Telegram UI and wallet UX out of `l2-node`.
- User onboarding flows.
- Public faucet integration.

### Что реализовать в ecosystem

- `ecosystem/wallet-telegram` future package.
- TON Connect integration.
- Account view.
- Deposit/transfer/withdraw flows.
- Testnet-only launch mode.

### Интерфейсы с l2-node

- Same public API as SDK.
- Faucet service API.
- Registry metadata.
- Explorer links.

### Пользовательский flow

- User opens Mini App.
- Connects TON wallet.
- Deposits TON/Jetton.
- Uses L2 account and transfers.
- Withdraws through TON wallet.

### Аудит и безопасность

- Telegram init data validation if backend used.
- No private key storage.
- No admin token in frontend.
- Testnet warnings.

### Тесты и проверки

- UI typecheck/build.
- Wallet mock tests.
- Payload tests.
- Manual Telegram testnet smoke later.

### Масштабируемость

- Reuse SDK and registry.
- Keep backend optional.
- Support multiple assets.

### Рефакторинг и чистота

- Do not fork SDK logic.
- Separate Telegram-specific code.
- Keep protocol calls typed.

### Acceptance criteria

- Future Mini App plan is clear.
- No current live prototype dependency.
- Security assumptions documented.

### Git/GitHub

- Branch: `docs(ecosystem): telegram-wallet-plan`.
- Commit: `docs(ecosystem): plan telegram wallet`.
- Push после checks.

## 18. Future indexer/exporter API for third-party explorers

### Цель

- Спланировать exporter API для third-party explorers and analytics.
- Не заставлять сторонние приложения читать DB напрямую.
- Подготовить stable read model beyond current explorer endpoints.

### Что вынести из L2

- Analytics/export formatting.
- Third-party explorer adapters.
- Bulk export tools.

### Что реализовать в ecosystem

- `ecosystem/exporter` plan.
- Public API schema for blocks, txs, deposits, withdrawals, commitments.
- Optional static snapshots.
- Webhook or stream consumer later.

### Интерфейсы с l2-node

- Read-only explorer API.
- WebSocket stream.
- Future paginated export endpoints.
- No DB/Redis direct access.

### Пользовательский flow

- Third-party explorer registers API URL/registry.
- Syncs blocks and txs.
- Verifies roots and links to L1.

### Аудит и безопасность

- Public data only.
- Pagination limits.
- No operator secrets.
- Avoid DoS through bulk export.

### Тесты и проверки

- Schema tests.
- Pagination tests.
- Mock sync test.
- Secret scan.

### Масштабируемость

- Cursor-based sync.
- Snapshots for large history.
- Rate limits.
- Future indexer service separate from node.

### Рефакторинг и чистота

- Keep exporter as ecosystem adapter.
- Core node only exposes stable API.
- Document compatibility.

### Acceptance criteria

- Third-party explorer integration path is documented.
- No direct database dependency.
- Future endpoint requirements are clear.

### Git/GitHub

- Branch: `docs(ecosystem): exporter-api-plan`.
- Commit: `docs(ecosystem): plan explorer exporter api`.
- Push после checks.

## Локальная проверка этого файла

- Выполнить `git status --short`.
- Ожидаемо: `?? ecosystem/PLAN.md` или staged/tracked только после явного `git add`.
- Выполнить `rg -n "^## [0-9]+\\." ecosystem/PLAN.md`.
- Ожидаемо: 18 задач.
- Проверить, что каждая задача содержит `Аудит и безопасность`, `Тесты и проверки`, `Масштабируемость`, `Рефакторинг и чистота`, `Git/GitHub`.
- Runtime tests не нужны, потому что этот шаг создает только roadmap.
- Не переносить текущие `sdk/` и `dashboard/` в этом шаге.
