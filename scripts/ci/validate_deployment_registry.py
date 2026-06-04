#!/usr/bin/env python3
"""Validate public Entropis deployment registries."""

from __future__ import annotations

import argparse
import copy
import json
import re
import sys
from pathlib import Path
from typing import Any


STATUSES = {"draft", "deployed", "verified", "deprecated"}
LIVE_STATUSES = {"deployed", "verified"}
ADDRESS_RE = re.compile(r"^-?\d+:[0-9a-fA-F]{64}$")
HASH_RE = re.compile(r"^0x[0-9a-fA-F]{64}$")
FORBIDDEN_KEY_PARTS = (
    "api_key",
    "apikey",
    "auth",
    "bearer",
    "database",
    "endpoint",
    "mnemonic",
    "password",
    "private",
    "redis",
    "seed",
    "secret",
    "token",
    "url",
)
FORBIDDEN_VALUE_PARTS = (
    "bearer ",
    "http://",
    "https://",
    "mnemonic",
    "postgres://",
    "postgresql://",
    "private key",
    "redis://",
    "seed phrase",
)


class RegistryError(ValueError):
    pass


def load_json(path: Path) -> dict[str, Any]:
    try:
        value = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        raise RegistryError(f"{path}: invalid JSON: {exc}") from exc
    if not isinstance(value, dict):
        raise RegistryError(f"{path}: registry root must be an object")
    return value


def walk(value: Any, path: str = "$") -> list[tuple[str, Any]]:
    items = [(path, value)]
    if isinstance(value, dict):
        for key, child in value.items():
            items.extend(walk(child, f"{path}.{key}"))
    elif isinstance(value, list):
        for index, child in enumerate(value):
            items.extend(walk(child, f"{path}[{index}]"))
    return items


def reject_private_fields(registry: dict[str, Any]) -> None:
    for path, value in walk(registry):
        key = path.rsplit(".", 1)[-1].lower()
        if any(part in key for part in FORBIDDEN_KEY_PARTS):
            raise RegistryError(f"{path}: private or endpoint-like field is not allowed")
        if isinstance(value, str):
            lowered = value.lower()
            if "mainnet" in lowered:
                raise RegistryError(f"{path}: mainnet values are not allowed")
            if any(part in lowered for part in FORBIDDEN_VALUE_PARTS):
                raise RegistryError(f"{path}: private endpoint or credential-like value is not allowed")


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RegistryError(message)


def optional_address(value: Any, path: str) -> None:
    if value is None:
        return
    require(isinstance(value, str) and ADDRESS_RE.fullmatch(value) is not None, f"{path}: invalid raw TON address")


def optional_hash(value: Any, path: str) -> None:
    if value is None:
        return
    require(isinstance(value, str) and HASH_RE.fullmatch(value) is not None, f"{path}: expected 0x-prefixed 32-byte hash")


def contract(registry_deployment: dict[str, Any], name: str) -> dict[str, Any]:
    contracts = registry_deployment.get("contracts")
    require(isinstance(contracts, dict), "deployment.contracts must be an object")
    value = contracts.get(name)
    require(isinstance(value, dict), f"deployment.contracts.{name} must be an object")
    return value


def validate_deployment(item: Any, active_id: str | None) -> None:
    require(isinstance(item, dict), "deployment entry must be an object")
    deployment_id = item.get("id")
    status = item.get("status")
    require(isinstance(deployment_id, str) and deployment_id, "deployment.id is required")
    require(status in STATUSES, f"{deployment_id}: invalid status")
    require(item.get("contractVersion"), f"{deployment_id}: contractVersion is required")

    parameters = item.get("parameters")
    require(isinstance(parameters, dict), f"{deployment_id}: parameters must be an object")
    challenge = parameters.get("challengeWindowSec")
    require(isinstance(challenge, int) and challenge > 0, f"{deployment_id}: challengeWindowSec must be positive")
    require(parameters.get("tonAssetId") == 1, f"{deployment_id}: TON asset id must be 1")
    require(parameters.get("tonDecimals") == 9, f"{deployment_id}: TON decimals must be 9")
    optional_address(parameters.get("sequencer"), f"{deployment_id}.parameters.sequencer")
    optional_address(parameters.get("wrappedGasMinter"), f"{deployment_id}.parameters.wrappedGasMinter")

    deployer = item.get("deployer")
    if isinstance(deployer, dict):
        optional_address(deployer.get("publicAddress"), f"{deployment_id}.deployer.publicAddress")

    root = contract(item, "RollupRoot")
    vault = contract(item, "AssetVault")
    for contract_name, value in (("RollupRoot", root), ("AssetVault", vault)):
        prefix = f"{deployment_id}.contracts.{contract_name}"
        optional_address(value.get("address"), f"{prefix}.address")
        optional_hash(value.get("codeHash"), f"{prefix}.codeHash")
        optional_hash(value.get("dataHash"), f"{prefix}.dataHash")
        optional_hash(value.get("deployTxHash"), f"{prefix}.deployTxHash")
        require(isinstance(value.get("expectedGetters"), dict), f"{prefix}.expectedGetters is required")

    txs = item.get("transactions", {})
    require(isinstance(txs, dict), f"{deployment_id}: transactions must be an object")
    for key, value in txs.items():
        optional_hash(value, f"{deployment_id}.transactions.{key}")

    verification = item.get("verification")
    require(isinstance(verification, dict), f"{deployment_id}: verification must be an object")

    if status in LIVE_STATUSES:
        require(item.get("deployedAt"), f"{deployment_id}: deployedAt is required for {status}")
        require(root.get("address") and vault.get("address"), f"{deployment_id}: live registry requires contract addresses")
        require(root.get("codeHash") and vault.get("codeHash"), f"{deployment_id}: live registry requires code hashes")
        require(root.get("deployTxHash") and vault.get("deployTxHash"), f"{deployment_id}: live registry requires deploy tx hashes")
        require(parameters.get("sequencer"), f"{deployment_id}: live registry requires sequencer address")
        require(isinstance(deployer, dict) and deployer.get("publicAddress"), f"{deployment_id}: live registry requires deployer public address")
        require(item.get("verification", {}).get("status") == "verified", f"{deployment_id}: live registry requires verified getters")

    if active_id == deployment_id:
        require(status in LIVE_STATUSES, f"{deployment_id}: active deployment must be deployed or verified")


def validate_registry(registry: dict[str, Any]) -> None:
    reject_private_fields(registry)
    require(registry.get("schemaVersion") == 1, "schemaVersion must be 1")
    require(registry.get("chainId") == "entropis-testnet", "chainId must be entropis-testnet")
    require(registry.get("tonNetwork") == "testnet", "tonNetwork must be testnet")

    active_id = registry.get("activeDeploymentId")
    require(active_id is None or isinstance(active_id, str), "activeDeploymentId must be string or null")
    deployments = registry.get("deployments")
    require(isinstance(deployments, list) and deployments, "deployments must be a non-empty list")

    seen: set[str] = set()
    for item in deployments:
        deployment_id = item.get("id") if isinstance(item, dict) else None
        validate_deployment(item, active_id)
        require(deployment_id not in seen, f"{deployment_id}: duplicate deployment id")
        seen.add(deployment_id)

    if active_id is not None:
        require(active_id in seen, "activeDeploymentId does not match any deployment")


def valid_draft() -> dict[str, Any]:
    return {
        "schemaVersion": 1,
        "chainId": "entropis-testnet",
        "tonNetwork": "testnet",
        "activeDeploymentId": None,
        "deployments": [
            {
                "id": "draft",
                "status": "draft",
                "contractVersion": "0.1.0",
                "parameters": {"challengeWindowSec": 300, "tonAssetId": 1, "tonDecimals": 9},
                "contracts": {
                    "RollupRoot": {"expectedGetters": {}, "address": None, "codeHash": None, "dataHash": None, "deployTxHash": None},
                    "AssetVault": {"expectedGetters": {}, "address": None, "codeHash": None, "dataHash": None, "deployTxHash": None},
                },
                "transactions": {},
                "verification": {"status": "not_deployed"},
            }
        ],
    }


def expect_failure(registry: dict[str, Any], label: str) -> None:
    try:
        validate_registry(registry)
    except RegistryError:
        return
    raise RegistryError(f"self-test failed: {label} unexpectedly passed")


def self_test() -> None:
    base = valid_draft()
    validate_registry(base)

    mainnet = copy.deepcopy(base)
    mainnet["tonNetwork"] = "mainnet"
    expect_failure(mainnet, "mainnet network")

    token = copy.deepcopy(base)
    token["signerToken"] = "replace-with-local-token"
    expect_failure(token, "token key")

    live_missing = copy.deepcopy(base)
    live_missing["activeDeploymentId"] = "draft"
    live_missing["deployments"][0]["status"] = "verified"
    expect_failure(live_missing, "verified deployment without public addresses")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("paths", nargs="*", default=["deployments/testnet/entropis.json"])
    parser.add_argument("--self-test", action="store_true", help="run validator negative tests")
    args = parser.parse_args()

    try:
        if args.self_test:
            self_test()
        for raw_path in args.paths:
            path = Path(raw_path)
            validate_registry(load_json(path))
            print(f"Deployment registry validation passed: {path.as_posix()}")
        if args.self_test:
            print("Deployment registry validator self-test passed.")
    except RegistryError as exc:
        print(f"Deployment registry validation failed: {exc}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
