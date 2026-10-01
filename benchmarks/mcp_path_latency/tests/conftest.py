from __future__ import annotations

import os
import signal
import subprocess
import sys
import time
from collections.abc import Callable, Iterator
from pathlib import Path
from tempfile import TemporaryDirectory
from uuid import uuid4

import pytest
import yaml

PROJECT = Path(__file__).resolve().parents[1]
Runner = Callable[[list[dict[str, object]]], tuple[subprocess.CompletedProcess[str], Path]]


@pytest.fixture
def benchmark_cli(tmp_path: Path) -> Iterator[Runner]:
    configured = os.environ.get("FUNCTERM_E2E_EXECUTABLE")
    if not configured:
        pytest.fail(
            "Set FUNCTERM_E2E_EXECUTABLE to the built FuncTerm binary before running E2E tests"
        )
    executable = Path(configured).resolve(strict=True)
    environment = {
        key: value for key, value in os.environ.items() if not key.upper().startswith("FUNCTERM_")
    }
    environment["PATH"] = os.pathsep.join(
        entry
        for entry in environment["PATH"].split(os.pathsep)
        if not (Path(entry).name == "shims" and "functerm" in Path(entry).parts)
    )
    environment.update(
        FUNCTERM_DAEMON_SERVICE_NAME=f"ft-e2e/{uuid4().hex}", FUNCTERM_DAEMON_READY_STDOUT="1"
    )
    startup_log = tmp_path / "daemon.log"
    with (
        TemporaryDirectory(prefix="ft-") as runtime,
        startup_log.open("w", encoding="utf-8") as log,
    ):
        environment.update(TMP=runtime, TEMP=runtime, TMPDIR=runtime)
        daemon = subprocess.Popen(
            [str(executable), "daemon"],
            env=environment,
            stdin=subprocess.DEVNULL,
            stdout=log,
            stderr=log,
            creationflags=subprocess.DETACHED_PROCESS if sys.platform == "win32" else 0,
            start_new_session=sys.platform != "win32",
        )
        try:
            deadline = time.monotonic() + 20
            while "Ready" not in startup_log.read_text(encoding="utf-8"):
                assert daemon.poll() is None, startup_log.read_text(encoding="utf-8")
                assert time.monotonic() < deadline, startup_log.read_text(encoding="utf-8")
                time.sleep(0.05)
            yield make_runner(tmp_path, executable, environment)
        finally:
            if daemon.poll() is None:
                if sys.platform == "win32":
                    subprocess.run(
                        ["taskkill.exe", "/PID", str(daemon.pid), "/T", "/F"],
                        check=True,
                        capture_output=True,
                        timeout=20,
                    )
                else:
                    os.killpg(daemon.pid, signal.SIGTERM)
            daemon.wait(timeout=20)


def make_runner(directory: Path, executable: Path, environment: dict[str, str]) -> Runner:

    def run(profiles: list[dict[str, object]]) -> tuple[subprocess.CompletedProcess[str], Path]:
        profile_directory = directory / "profiles"
        profile_directory.mkdir()
        for index, profile in enumerate(profiles):
            with (profile_directory / f"{index}.yaml").open("w", encoding="utf-8") as profile_file:
                yaml.safe_dump(profile, profile_file)
        settings = {
            "clients": {"loop_count": 1, "list_tools_on_connect": True},
            "server": {
                "command": str(executable),
                "args": ["mcp"],
                "cwd": str(directory),
                "env": environment,
            },
            "report": {
                "directory": "reports",
                "summary_file": "summary.json",
                "events_file": "events.jsonl",
            },
        }
        configuration = directory / "settings.yaml"
        with configuration.open("w", encoding="utf-8") as settings_file:
            yaml.safe_dump(settings, settings_file)
        result = subprocess.run(
            [sys.executable, "-m", "functerm_mcp_path_latency.cli", "--config", str(configuration)],
            cwd=PROJECT,
            env=environment,
            capture_output=True,
            text=True,
            encoding="utf-8",
            timeout=60,
            check=False,
        )
        return result, directory / "reports"

    return run
