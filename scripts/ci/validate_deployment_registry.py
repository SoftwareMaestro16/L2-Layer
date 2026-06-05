#!/usr/bin/env python3
"""Validate the public testnet deployment registry is public metadata only."""

from __future__ import annotations

import argparse
import json
import re
import sys
from collections.abc import Iterator
from pathlib import Path
from typing import Any


REGISTRY_PATH = Path("deployments/testnet/entropis.json")
SECRET_KEY = re.compile(
    r"(token|secret|mnemonic|private|seed|password|api[_-]?key|database[_-]?url|"
    r"redis[_-]?url|signed[_-]?boc|raw[_-]?boc|endpoint)",
    re.IGNORECASE,
)
SECRET_VALUE = re.compile(
    r"(postgres(?:ql)?://|redis://|bearer\s+[a-z0-9._~+/=-]{8,}|"
    r"-----BEGIN [A-Z ]+PRIVATE KEY-----|toncenter_api_key|tonapi_key)",
    re.IGNORECASE,
)


def walk(value: Any, path: str = "$") -> Iterator[tuple[str, Any]]:
    yield path, value
    if isinstance(value, dict):
        for key, item in value.items():
            yield from walk(item, f"{path}.{key}")
    elif isinstance(value, list):
        for index, item in enumerate(value):
            yield from walk(item, f"{path}[{index}]")


def validate_registry(data: dict[str, Any]) -> list[str]:
    errors: list[str] = []
    required = {"schemaVersion", "network", "chainId", "status", "deployments"}
    missing = sorted(required - data.keys())
    if missing:
        errors.append(f"missing required fields: {', '.join(missing)}")
    if data.get("schemaVersion") != 1:
        errors.append("schemaVersion must be 1")
    if data.get("network") != "testnet":
        errors.append("network must be testnet")
    if data.get("chainId") != "entropis-testnet":
        errors.append("chainId must be entropis-testnet")
    if data.get("status") not in {"draft", "deployed", "verified", "deprecated"}:
        errors.append("status must be draft, deployed, verified, or deprecated")
    deployments = data.get("deployments")
    if not isinstance(deployments, list):
        errors.append("deployments must be an array")
    elif data.get("status") in {"deployed", "verified"} and not deployments:
        errors.append("deployed/verified registry must include deployments")

    active = data.get("activeDeploymentId")
    if active is not None and deployments:
        ids = {item.get("id") for item in deployments if isinstance(item, dict)}
        if active not in ids:
            errors.append("activeDeploymentId must reference a deployment id")

    for path, value in walk(data):
        key = path.rsplit(".", 1)[-1].split("[", 1)[0]
        if SECRET_KEY.search(key):
            errors.append(f"{path} uses forbidden private field name")
        if isinstance(value, str):
            lowered = value.lower()
            if "mainnet" in lowered:
                errors.append(f"{path} must not reference mainnet")
            if SECRET_VALUE.search(value):
                errors.append(f"{path} looks like secret material")
    return errors


def load_json(path: Path) -> dict[str, Any]:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as error:
        raise SystemExit(f"{path}: invalid JSON: {error}") from error
    if not isinstance(data, dict):
        raise SystemExit(f"{path}: registry root must be an object")
    return data


def self_test() -> int:
    good = {
        "schemaVersion": 1,
        "network": "testnet",
        "chainId": "entropis-testnet",
        "status": "draft",
        "activeDeploymentId": None,
        "deployments": [],
    }
    bad = {
        **good,
        "status": "verified",
        "providerApiKey": "abc123",
        "deployments": [],
    }
    if validate_registry(good):
        print("self-test good fixture failed", file=sys.stderr)
        return 1
    if not validate_registry(bad):
        print("self-test bad fixture passed unexpectedly", file=sys.stderr)
        return 1
    print("Deployment registry validator self-test passed.")
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("path", nargs="?", default=str(REGISTRY_PATH))
    parser.add_argument("--self-test", action="store_true")
    args = parser.parse_args()

    if args.self_test:
        return self_test()

    path = Path(args.path)
    data = load_json(path)
    errors = validate_registry(data)
    if errors:
        print(f"{path}: deployment registry validation failed:", file=sys.stderr)
        for error in errors:
            print(f"  - {error}", file=sys.stderr)
        return 1
    print(f"{path}: deployment registry validation passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
