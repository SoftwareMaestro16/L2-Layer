# Entropis L1 TSA Checker Pack

Status: tracked checker sources only. Do not commit generated `.boc`, SARIF,
coverage, or exported-input artifacts.

This pack is the starting point for RollupRoot / AssetVault symbolic checks.
It does not replace Acton tests and does not prove fraud-proof correctness. It
adds a reproducible TSA entrypoint for contract-drain and bounce-path analysis.

## Checkers

- `checkers/no_excessive_refund.fc`
  Checks that a single internal input cannot make the analyzed contract send
  more TON back to the original sender than the sender attached.
- `checkers/no_bounce_reentrant_send.fc`
  Checks that bounced-message handling does not fail and does not emit further
  messages from the analyzed contract.

## Local Availability

```powershell
py -3 scripts\ci\tsa_l1_custom_check.py
```

The default mode verifies TSA availability and checker assets only.

## Running Against Local BoCs

Generate code/data BoCs under ignored `build/tsa/`, then pass paths explicitly:

```powershell
py -3 scripts\ci\tsa_l1_custom_check.py `
  --checker no_excessive_refund `
  --code-boc build\tsa\rollup-root-code.boc `
  --data-boc build\tsa\rollup-root-data.boc `
  --balance 1000000000 `
  --address 0:1111111111111111111111111111111111111111111111111111111111111111
```

Use a short timeout first. TSA reports must be validated with ordinary Acton
tests before any issue is classified as a finding.
