#!/usr/bin/env python3
"""Run or validate Entropis L1 TSA custom checker assets."""

from __future__ import annotations

import argparse
import json
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[2]
CHECKERS = {
    "no_excessive_refund": ROOT / "tsa-analysis" / "checkers" / "no_excessive_refund.fc",
    "no_bounce_reentrant_send": ROOT
    / "tsa-analysis"
    / "checkers"
    / "no_bounce_reentrant_send.fc",
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--checker", choices=CHECKERS.keys(), default="no_excessive_refund")
    parser.add_argument("--code-boc", type=Path)
    parser.add_argument("--data-boc", type=Path)
    parser.add_argument("--balance", default="-")
    parser.add_argument("--address", default="-")
    parser.add_argument("--timeout", default="30")
    args = parser.parse_args()

    location = tsa_location()
    validate_checker_assets()

    if args.code_boc is None:
        print("TSA L1 checker assets: PASS")
        print("No --code-boc supplied; symbolic run skipped.")
        return 0

    if args.data_boc is None:
        print("--data-boc is required when --code-boc is supplied", file=sys.stderr)
        return 1
    if not args.code_boc.exists() or not args.data_boc.exists():
        print("code/data BoC path does not exist", file=sys.stderr)
        return 1

    return run_tsa(location, args)


def tsa_location() -> dict[str, Any]:
    npm = shutil.which("npm")
    if npm is None:
        raise SystemExit("npm is required for tsa-installer")

    result = subprocess.run(
        [
            npm,
            "exec",
            "--yes",
            "--package",
            "tsa-installer",
            "--",
            "tsa-installer",
            "install",
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=180,
    )
    if result.returncode != 0:
        print((result.stdout + result.stderr).strip(), file=sys.stderr)
        raise SystemExit(result.returncode)

    text = result.stdout + result.stderr
    location = last_json_object(text)
    if location.get("installed") is not True:
        raise SystemExit("tsa-installer reported installed=false")
    return location


def validate_checker_assets() -> None:
    missing = [path for path in CHECKERS.values() if not path.exists()]
    if missing:
        raise SystemExit(f"missing TSA checker assets: {missing}")

    for path in CHECKERS.values():
        text = path.read_text(encoding="utf-8")
        if "tsa_" not in text or "main()" not in text:
            raise SystemExit(f"checker does not look like a TSA checker: {path}")


def run_tsa(location: dict[str, Any], args: argparse.Namespace) -> int:
    checker = CHECKERS[args.checker]
    with tempfile.TemporaryDirectory(prefix="entropis-tsa-") as tmp_name:
        tmp = Path(tmp_name)
        tmp_checker_dir = tmp / "checkers"
        tmp_imports = tmp_checker_dir / "imports"
        tmp_imports.mkdir(parents=True)
        shutil.copy2(checker, tmp_checker_dir / checker.name)
        shutil.copy2(Path(location["func_imports"]) / "stdlib.fc", tmp_imports / "stdlib.fc")
        shutil.copy2(
            Path(location["func_imports"]) / "tsa_functions.fc",
            tmp_imports / "tsa_functions.fc",
        )

        command = [
            location["java"],
            "-jar",
            location["location"],
            "custom-checker",
            "--checker",
            str(tmp_checker_dir / checker.name),
            "--fift-std",
            location["fiftstdlib"],
            "--timeout",
            args.timeout,
            "-c",
            f"Boc {args.code_boc}",
            "-d",
            str(args.data_boc),
            "-b",
            args.balance,
            "-a",
            args.address,
        ]
        result = subprocess.run(command, cwd=ROOT, text=True, check=False)
        return result.returncode


def last_json_object(output: str) -> dict[str, Any]:
    decoder = json.JSONDecoder()
    found: dict[str, Any] | None = None
    for index, char in enumerate(output):
        if char != "{":
            continue
        try:
            value, _ = decoder.raw_decode(output[index:])
        except json.JSONDecodeError:
            continue
        if isinstance(value, dict):
            found = value
    if found is None:
        raise SystemExit("tsa-installer did not print a JSON location object")
    return found


if __name__ == "__main__":
    raise SystemExit(main())
