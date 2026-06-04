#!/usr/bin/env python3
"""Fail CI if tracked or staged files contain obvious live secrets."""

from __future__ import annotations

import argparse
import re
import subprocess
import sys
from pathlib import Path


SKIP_SUFFIXES = {
    ".png",
    ".jpg",
    ".jpeg",
    ".gif",
    ".webp",
    ".ico",
    ".pdf",
    ".zip",
}

SKIP_DIRS = {
    ".git",
    ".acton",
    "build",
    "gen",
    "node_modules",
    "target",
}

PATTERNS = [
    ("redis url with password", re.compile(r"\bredis://[^:\s/@]+:[^@\s]+@[^\s]+", re.I)),
    (
        "postgres url with password",
        re.compile(r"\bpostgres(?:ql)?://[^:\s/@]+:[^@\s]+@[^\s]+", re.I),
    ),
    (
        "non-placeholder env secret",
        re.compile(
            r"\b(?:TONCENTER_API_KEY|TONAPI_KEY|L2_ADMIN_TOKEN|L1_COMMIT_SIGNER_TOKEN|L2_SIGNER_TOKEN)"
            r"\s*=\s*([^\s]+)",
        ),
    ),
    (
        "private key or mnemonic assignment",
        re.compile(
            r"\b(?:mnemonic|private[_-]?key|secret[_-]?key|seed[_-]?phrase)"
            r"\s*[:=]\s*['\"]?([A-Za-z0-9_./+=-]{12,})",
            re.I,
        ),
    ),
    ("Railway proxy endpoint", re.compile(r"\b[a-z0-9-]+\.proxy\.rlwy\.net:\d+\b", re.I)),
]


def git_files(staged: bool) -> list[Path]:
    command = ["git", "diff", "--cached", "--name-only", "-z"] if staged else ["git", "ls-files", "-z"]
    result = subprocess.run(command, check=True, stdout=subprocess.PIPE)
    return [Path(name) for name in result.stdout.decode("utf-8").split("\0") if name]


def is_skipped(path: Path) -> bool:
    if path.suffix.lower() in SKIP_SUFFIXES:
        return True
    return any(part in SKIP_DIRS for part in path.parts)


def allowed_placeholder(line: str) -> bool:
    lowered = line.lower()
    placeholders = (
        "replace-with",
        "password@host",
        "user:password@host",
        "localhost",
        "127.0.0.1",
        "=<",
        ": <",
    )
    return any(marker in lowered for marker in placeholders)


def scan_file(path: Path) -> list[str]:
    if is_skipped(path) or not path.exists():
        return []

    try:
        text = path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return []

    findings: list[str] = []
    for line_no, line in enumerate(text.splitlines(), start=1):
        if allowed_placeholder(line):
            continue
        for label, pattern in PATTERNS:
            if pattern.search(line):
                findings.append(f"{path.as_posix()}:{line_no}: {label}")
    return findings


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--staged", action="store_true", help="scan only staged files")
    args = parser.parse_args()

    findings: list[str] = []
    for path in git_files(args.staged):
        findings.extend(scan_file(path))

    if findings:
        print("Secret scan failed. Remove or rotate these values before pushing:", file=sys.stderr)
        for finding in findings:
            print(f"  - {finding}", file=sys.stderr)
        return 1

    print("Secret scan passed.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
