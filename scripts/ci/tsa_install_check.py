#!/usr/bin/env python3
"""Check that TON Symbolic Analyzer tooling can be installed and located.

The older `npx tsa-installer install` path can fail on some npm setups with
`cb.apply is not a function`. This check uses `npm exec --package` so CI and
local Windows/WSL runs have a stable TSA availability gate.
"""

from __future__ import annotations

import json
import os
import shutil
import subprocess
import sys
from typing import Any


def main() -> int:
    npm = shutil.which("npm")
    if npm is None:
        print("npm is required to install/check tsa-installer", file=sys.stderr)
        return 1

    command = [
        npm,
        "exec",
        "--yes",
        "--package",
        "tsa-installer",
        "--",
        "tsa-installer",
        "install",
    ]
    timeout = int(os.environ.get("TSA_INSTALL_TIMEOUT_SEC", "180"))
    result = subprocess.run(
        command,
        capture_output=True,
        text=True,
        timeout=timeout,
        check=False,
    )

    combined_output = "\n".join(part for part in [result.stdout, result.stderr] if part)
    if result.returncode != 0:
        print(combined_output.strip(), file=sys.stderr)
        return result.returncode

    try:
        location = last_json_object(combined_output)
    except ValueError as error:
        print(str(error), file=sys.stderr)
        print(combined_output.strip(), file=sys.stderr)
        return 1

    required_keys = ["location", "java", "fiftstdlib", "func_imports", "installed"]
    missing = [key for key in required_keys if key not in location]
    if missing:
        print(f"tsa-installer output missing keys: {', '.join(missing)}", file=sys.stderr)
        return 1
    if location["installed"] is not True:
        print("tsa-installer reported installed=false", file=sys.stderr)
        return 1

    print("TSA availability: PASS")
    print(f"tsa-cli: {location['location']}")
    print(f"java: {location['java']}")
    print(f"fiftstdlib: {location['fiftstdlib']}")
    print(f"func_imports: {location['func_imports']}")
    return 0


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
        raise ValueError("tsa-installer did not print a JSON location object")
    return found


if __name__ == "__main__":
    raise SystemExit(main())
