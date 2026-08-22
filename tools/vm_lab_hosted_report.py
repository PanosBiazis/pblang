#!/usr/bin/env python3
"""Create and verify content-bound evidence for the hosted VM lab."""

from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import sys


FORMAT = "pblang.vm-lab.hosted-report.v2"
NAME_RE = re.compile(r"^[a-z0-9_-]+$")
HEX_RE = re.compile(r"^[0-9a-f]+$")


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest()


def source_tree_sha256() -> str:
    # The index manifest binds every tracked path, mode, stage, and Git blob ID.
    # Hosted checkouts are also required to be clean before evidence is accepted.
    manifest = subprocess.check_output(["git", "ls-files", "-s", "-z"])
    return hashlib.sha256(manifest).hexdigest()


def parse_named(value: str, separator: str = "=") -> tuple[str, str]:
    name, found, data = value.partition(separator)
    if not found or not NAME_RE.fullmatch(name):
        raise ValueError(f"invalid named value: {value!r}")
    return name, data


def parse_bool(value: str) -> bool:
    if value == "true":
        return True
    if value == "false":
        return False
    raise ValueError("expected true or false")


def canonical_sha256(payload: dict[str, object]) -> str:
    encoded = json.dumps(payload, sort_keys=True, separators=(",", ":")).encode("utf-8")
    return hashlib.sha256(encoded).hexdigest()


def record(args: argparse.Namespace) -> int:
    checks: list[dict[str, object]] = []
    for value in args.check:
        name, rest = parse_named(value)
        status_text, found, log_text = rest.partition(":")
        if not found:
            raise ValueError(f"check must be NAME=STATUS:LOG: {value!r}")
        status = int(status_text)
        log = Path(log_text)
        checks.append(
            {
                "name": name,
                "status": status,
                "ok": status == 0,
                "log_bytes": log.stat().st_size if log.is_file() else None,
                "log_sha256": sha256_file(log) if log.is_file() else None,
            }
        )

    artifacts: list[dict[str, object]] = []
    for value in args.artifact:
        name, path_text = parse_named(value)
        path = Path(path_text)
        exists = path.is_file()
        artifacts.append(
            {
                "name": name,
                "path": path.as_posix(),
                "present": exists,
                "bytes": path.stat().st_size if exists else None,
                "sha256": sha256_file(path) if exists else None,
            }
        )

    metadata: dict[str, str] = {}
    for value in args.metadata:
        name, data = parse_named(value)
        metadata[name] = data

    cleanup_required = parse_bool(args.cleanup_required)
    cleanup_verified = parse_bool(args.cleanup_verified)
    ok = (
        bool(checks)
        and all(bool(item["ok"]) for item in checks)
        and all(item["log_sha256"] is not None for item in checks)
        and bool(artifacts)
        and all(bool(item["present"]) for item in artifacts)
        and (not cleanup_required or cleanup_verified)
    )
    payload: dict[str, object] = {
        "format": FORMAT,
        "target": args.target,
        "evidence": args.evidence,
        "source_revision": subprocess.check_output(
            ["git", "rev-parse", "HEAD"], text=True
        ).strip(),
        "source_tree_sha256": source_tree_sha256(),
        "runner": {
            "os": os.environ.get("RUNNER_OS", sys.platform),
            "arch": os.environ.get("RUNNER_ARCH", "unknown"),
            "image": os.environ.get("ImageOS", "unknown"),
        },
        "checks": checks,
        "artifacts": artifacts,
        "metadata": metadata,
        "cleanup_required": cleanup_required,
        "cleanup_verified": cleanup_verified,
        "ok": ok,
    }
    payload["report_sha256"] = canonical_sha256(payload)
    output = Path(args.output)
    output.parent.mkdir(parents=True, exist_ok=True)
    output.write_text(
        json.dumps(payload, sort_keys=True, separators=(",", ":")) + "\n",
        encoding="utf-8",
        newline="\n",
    )
    return 0


def is_digest(value: object, lengths: tuple[int, ...] = (64,)) -> bool:
    return (
        isinstance(value, str)
        and len(value) in lengths
        and HEX_RE.fullmatch(value) is not None
    )


def is_safe_relative_path(value: object) -> bool:
    if not isinstance(value, str) or not value or "\\" in value:
        return False
    path = Path(value)
    return not path.is_absolute() and ".." not in path.parts


def valid_payload(payload: object) -> bool:
    if not isinstance(payload, dict):
        return False
    expected_keys = {
        "format", "target", "evidence", "source_revision", "source_tree_sha256",
        "runner", "checks", "artifacts", "metadata", "cleanup_required",
        "cleanup_verified", "ok",
    }
    if set(payload) != expected_keys:
        return False
    if payload.get("format") != FORMAT:
        return False
    if not isinstance(payload.get("target"), str) or not NAME_RE.fullmatch(payload["target"]):
        return False
    if not isinstance(payload.get("evidence"), str) or not NAME_RE.fullmatch(payload["evidence"]):
        return False
    if not is_digest(payload.get("source_revision"), (40, 64)):
        return False
    if not is_digest(payload.get("source_tree_sha256")):
        return False

    runner = payload.get("runner")
    if not isinstance(runner, dict) or set(runner) != {"os", "arch", "image"}:
        return False
    if not all(isinstance(value, str) and value for value in runner.values()):
        return False

    checks = payload.get("checks")
    if not isinstance(checks, list) or not checks:
        return False
    check_names: set[str] = set()
    for item in checks:
        if not isinstance(item, dict) or set(item) != {
            "name", "status", "ok", "log_bytes", "log_sha256"
        }:
            return False
        name = item.get("name")
        status = item.get("status")
        if not isinstance(name, str) or not NAME_RE.fullmatch(name) or name in check_names:
            return False
        check_names.add(name)
        if isinstance(status, bool) or not isinstance(status, int):
            return False
        if item.get("ok") is not (status == 0):
            return False
        if isinstance(item.get("log_bytes"), bool) or not isinstance(item.get("log_bytes"), int) or item["log_bytes"] < 0:
            return False
        if not is_digest(item.get("log_sha256")):
            return False

    artifacts = payload.get("artifacts")
    if not isinstance(artifacts, list) or not artifacts:
        return False
    artifact_names: set[str] = set()
    for item in artifacts:
        if not isinstance(item, dict) or set(item) != {
            "name", "path", "present", "bytes", "sha256"
        }:
            return False
        name = item.get("name")
        if not isinstance(name, str) or not NAME_RE.fullmatch(name) or name in artifact_names:
            return False
        artifact_names.add(name)
        if item.get("present") is not True or not is_safe_relative_path(item.get("path")):
            return False
        if isinstance(item.get("bytes"), bool) or not isinstance(item.get("bytes"), int) or item["bytes"] < 0:
            return False
        if not is_digest(item.get("sha256")):
            return False

    metadata = payload.get("metadata")
    if not isinstance(metadata, dict) or not all(
        isinstance(key, str) and NAME_RE.fullmatch(key) and isinstance(value, str)
        for key, value in metadata.items()
    ):
        return False
    cleanup_required = payload.get("cleanup_required")
    cleanup_verified = payload.get("cleanup_verified")
    if not isinstance(cleanup_required, bool) or not isinstance(cleanup_verified, bool):
        return False
    computed_ok = (
        all(item["ok"] is True for item in checks)
        and all(item["present"] is True for item in artifacts)
        and (not cleanup_required or cleanup_verified)
    )
    return payload.get("ok") is True and computed_ok


def verify(args: argparse.Namespace) -> int:
    payload = json.loads(Path(args.report).read_text(encoding="utf-8"))
    if not isinstance(payload, dict):
        return 1
    expected = payload.pop("report_sha256", None)
    actual = canonical_sha256(payload)
    if not is_digest(expected) or expected != actual or not valid_payload(payload):
        return 1
    return 0


def parser() -> argparse.ArgumentParser:
    root = argparse.ArgumentParser()
    commands = root.add_subparsers(dest="command", required=True)
    create = commands.add_parser("record")
    create.add_argument("--target", required=True)
    create.add_argument("--evidence", required=True)
    create.add_argument("--output", required=True)
    create.add_argument("--check", action="append", default=[])
    create.add_argument("--artifact", action="append", default=[])
    create.add_argument("--metadata", action="append", default=[])
    create.add_argument("--cleanup-required", default="false")
    create.add_argument("--cleanup-verified", default="true")
    create.set_defaults(handler=record)
    validate = commands.add_parser("verify")
    validate.add_argument("report")
    validate.set_defaults(handler=verify)
    return root


def main() -> int:
    args = parser().parse_args()
    try:
        return args.handler(args)
    except (OSError, ValueError, json.JSONDecodeError, subprocess.SubprocessError) as error:
        print(f"vm-lab-report: {error}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    raise SystemExit(main())
