#!/usr/bin/env python3
"""Fail CI if tracked or staged files include local artifacts or credentials."""

from __future__ import annotations

import argparse
import fnmatch
import subprocess
import sys
from pathlib import Path


ALLOW = {
    ".env.example",
    ".env.template",
    "deployments/testnet/entropis.json",
    "deployments/testnet/entropis.schema.json",
}

FORBIDDEN = (
    ".env",
    ".env.*",
    "*.pem",
    "*.key",
    "*.p12",
    "*.pfx",
    "*.mnemonic",
    "*.seed",
    "*.secret",
    "*.wallet",
    "*mnemonic*",
    "*seed-phrase*",
    "*private-key*",
    "*secret-key*",
    "keys/**",
    "secrets/**",
    "wallets/**",
    "wallets.toml",
    "global.wallets.toml",
    "libraries.toml",
    ".acton/**",
    "build/**",
    "deployments/**",
    "gen/**",
    "target/**",
    "**/target/**",
    "node_modules/**",
    "**/node_modules/**",
    "sdk/dist/**",
    "sdk/coverage/**",
    "data/**",
    "tmp/**",
    "temp/**",
    "*.db",
    "*.sqlite",
    "*.sqlite3",
    "*.boc",
    "*.fif",
    "*.fc.map",
    "*.source.map",
    "*.deployment.json",
    ".codex/skills/cosmos-vulnerability-scanner/**",
)


def git_files(staged: bool) -> list[str]:
    command = ["git", "diff", "--cached", "--name-only", "-z"] if staged else ["git", "ls-files", "-z"]
    result = subprocess.run(command, check=True, stdout=subprocess.PIPE)
    return [name for name in result.stdout.decode("utf-8").split("\0") if name]


def is_forbidden(path: str) -> bool:
    normalized = Path(path).as_posix()
    if normalized in ALLOW:
        return False
    return any(fnmatch.fnmatchcase(normalized, pattern) for pattern in FORBIDDEN)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--staged", action="store_true", help="check only staged files")
    args = parser.parse_args()

    violations = [path for path in git_files(args.staged) if is_forbidden(path)]
    if violations:
        print("Artifact guard failed. These files must not be committed:", file=sys.stderr)
        for path in violations:
            print(f"  - {path}", file=sys.stderr)
        return 1

    print("Artifact guard passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
