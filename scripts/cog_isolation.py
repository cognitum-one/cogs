#!/usr/bin/env python3
"""Run a Cog under its declared console limits and emit bounded evidence."""

from __future__ import annotations

import argparse
import json
import os
import re
import shlex
import signal
import subprocess
import threading
import time
import tomllib
from pathlib import Path
from typing import Any

COG_ID = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")
MAX_MANIFEST_BYTES = 256 * 1024
MAX_COMMANDS = 32
MAX_RUNTIME_SECS = 300
MAX_OUTPUT_BYTES = 1024 * 1024


class IsolationError(ValueError):
    """A manifest or execution cannot produce valid isolation evidence."""


def load_policy(path: Path) -> dict[str, Any]:
    if not path.is_file() or path.stat().st_size > MAX_MANIFEST_BYTES:
        raise IsolationError("manifest is missing or too large")
    try:
        manifest = tomllib.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, tomllib.TOMLDecodeError) as error:
        raise IsolationError("manifest is not valid UTF-8 TOML") from error
    console = manifest.get("console")
    if not isinstance(console, dict):
        raise IsolationError("manifest has no [console] policy")
    allowed = console.get("allowed_commands")
    runtime = console.get("max_runtime_secs")
    output = console.get("output_limit_bytes")
    if not isinstance(allowed, list) or not 1 <= len(allowed) <= MAX_COMMANDS:
        raise IsolationError("allowed_commands must contain 1-32 exact commands")
    for command in allowed:
        if (
            not isinstance(command, str)
            or not 1 <= len(command) <= 512
            or any(
                ord(character) < 0x20 or ord(character) > 0x7E for character in command
            )
        ):
            raise IsolationError("allowed command is malformed")
        try:
            arguments = shlex.split(command, posix=True)
        except ValueError as error:
            raise IsolationError("allowed command has invalid quoting") from error
        if not arguments or any("\x00" in argument for argument in arguments):
            raise IsolationError("allowed command has no safe arguments")
    if len(set(allowed)) != len(allowed):
        raise IsolationError("allowed_commands contains duplicates")
    if not isinstance(runtime, int) or not 1 <= runtime <= MAX_RUNTIME_SECS:
        raise IsolationError("max_runtime_secs must be between 1 and 300")
    if not isinstance(output, int) or not 1 <= output <= MAX_OUTPUT_BYTES:
        raise IsolationError("output_limit_bytes must be between 1 and 1048576")
    return {
        "allowed_commands": allowed,
        "max_runtime_secs": runtime,
        "output_limit_bytes": output,
    }


def refused(command: str, policy: dict[str, Any]) -> dict[str, Any]:
    if command in policy["allowed_commands"]:
        raise IsolationError("negative-control command is unexpectedly allowlisted")
    return {
        "command": command,
        "exit": 4,
        "refused": True,
        "spawned": False,
        "evidence": None,
    }


def run_allowed(binary: Path, command: str, policy: dict[str, Any]) -> dict[str, Any]:
    if command not in policy["allowed_commands"]:
        return refused(command, policy)
    arguments = shlex.split(command, posix=True)
    started = time.monotonic()
    process = subprocess.Popen(
        [str(binary), *arguments],
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    limit = policy["output_limit_bytes"]
    state: dict[str, Any] = {"bytes": 0, "overflow": False}
    lock = threading.Lock()

    def drain(stream: Any) -> None:
        while True:
            chunk = stream.read(4096)
            if not chunk:
                return
            with lock:
                state["bytes"] += len(chunk)
                if state["bytes"] > limit:
                    state["overflow"] = True
            if state["overflow"]:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                return

    readers = [
        threading.Thread(target=drain, args=(stream,), daemon=True)
        for stream in (process.stdout, process.stderr)
    ]
    for reader in readers:
        reader.start()
    timed_out = False
    try:
        exit_code = process.wait(timeout=policy["max_runtime_secs"])
    except subprocess.TimeoutExpired:
        timed_out = True
        os.killpg(process.pid, signal.SIGKILL)
        exit_code = process.wait()
    for reader in readers:
        reader.join(timeout=1)
    for stream in (process.stdout, process.stderr):
        if stream is not None:
            stream.close()
    elapsed_ms = int((time.monotonic() - started) * 1000)
    evidence = {
        "exit_code": exit_code,
        "killed_at_deadline": timed_out,
        "elapsed_ms": elapsed_ms,
        "max_runtime_secs": policy["max_runtime_secs"],
        "output_bytes": min(state["bytes"], limit),
        "output_limit_bytes": limit,
        "output_truncated": state["overflow"],
        "within_runtime_limit": not timed_out,
        "within_output_limit": not state["overflow"],
    }
    return {"command": command, "exit": exit_code, "evidence": evidence}


def gather(cog_id: str, manifest: Path, binary: Path) -> dict[str, Any]:
    if not COG_ID.fullmatch(cog_id):
        raise IsolationError("cog id is malformed")
    if not binary.is_file():
        raise IsolationError("native Cog binary is missing")
    policy = load_policy(manifest)
    runs = [
        run_allowed(binary, command, policy) for command in policy["allowed_commands"]
    ]
    runs.append(run_allowed(binary, "--definitely-not-allowed", policy))
    passed = all(
        row.get("refused") is True
        or (
            row.get("exit") == 0
            and row.get("evidence", {}).get("within_runtime_limit") is True
            and row.get("evidence", {}).get("within_output_limit") is True
        )
        for row in runs
    )
    return {
        "schema": "cognitum.cog.isolation-evidence.v1",
        "cogId": cog_id,
        "runOnTarget": False,
        "policy": policy,
        "runs": runs,
        "passed": passed,
    }


def evidence_passed(data: dict[str, Any]) -> bool:
    runs = data.get("runs")
    policy = data.get("policy")
    if (
        data.get("schema") != "cognitum.cog.isolation-evidence.v1"
        or data.get("passed") is not True
        or data.get("runOnTarget") is not False
        or not isinstance(policy, dict)
        or not isinstance(policy.get("allowed_commands"), list)
        or not policy.get("allowed_commands")
        or not isinstance(runs, list)
    ):
        return False
    allowed = policy["allowed_commands"]
    controls = [
        row
        for row in runs
        if isinstance(row, dict) and row.get("command") == "--definitely-not-allowed"
    ]
    positives = [
        row
        for row in runs
        if isinstance(row, dict) and row.get("command") != "--definitely-not-allowed"
    ]
    if (
        len(controls) != 1
        or len(runs) != len(allowed) + 1
        or controls[0]
        != {
            "command": "--definitely-not-allowed",
            "exit": 4,
            "refused": True,
            "spawned": False,
            "evidence": None,
        }
        or [row.get("command") for row in positives] != allowed
    ):
        return False
    for row in positives:
        if not isinstance(row, dict):
            return False
        evidence = row.get("evidence")
        if (
            row.get("exit") != 0
            or not isinstance(evidence, dict)
            or evidence.get("within_runtime_limit") is not True
            or evidence.get("within_output_limit") is not True
        ):
            return False
    return True


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cog-id", required=True)
    parser.add_argument("--manifest", required=True, type=Path)
    parser.add_argument("--binary", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    try:
        evidence = gather(args.cog_id, args.manifest, args.binary)
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(
            json.dumps(evidence, sort_keys=True, indent=2) + "\n", encoding="utf-8"
        )
        if not evidence["passed"]:
            raise IsolationError(
                "one or more allowed commands failed or breached a limit"
            )
    except (IsolationError, OSError) as error:
        print(f"isolation error: {error}", file=__import__("sys").stderr)
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
