import json
import sys
from collections.abc import Callable
from pathlib import Path
from subprocess import CompletedProcess

import pytest


@pytest.mark.parametrize("combined_profile", [False, True])
def test_real_tool_failure_returns_nonzero_and_preserves_successful_steps(
    benchmark_cli: Callable[[list[dict[str, object]]], tuple[CompletedProcess[str], Path]],
    combined_profile: bool,
) -> None:
    create_step: dict[str, object] = {
        "name": "create",
        "tool": "new_tab",
        "arguments": {
            "exec": [{"starting_shell": "powershell" if sys.platform == "win32" else "bash"}]
        },
        "capture": {"tab": "TAB_ID"},
    }
    failing_step: dict[str, object] = {
        "name": "invalid-view",
        "tool": "view",
        "arguments": {"ids": ["missing-e2e-command"], "wait_timeout": 0},
    }
    profiles: list[dict[str, object]]
    if combined_profile:
        profiles = [
            {"name": "partially-completed-profile", "scenario": [create_step, failing_step]}
        ]
    else:
        profiles = [
            {"name": "completed-profile", "scenario": [create_step]},
            {"name": "failing-profile", "scenario": [failing_step]},
        ]
    result, reports = benchmark_cli(profiles)
    assert result.returncode == 1, result.stdout + result.stderr
    summary = json.loads((reports / "summary.json").read_text(encoding="utf-8"))
    events = [
        json.loads(line)
        for line in (reports / "events.jsonl").read_text(encoding="utf-8").splitlines()
    ]
    assert summary["failed_clients"] == len(profiles)
    assert summary["succeeded_clients"] == 0
    for client in summary["clients"]:
        assert "missing-e2e-command" in client["error"]
        assert not client["succeeded"]
    assert events == [event for client in summary["clients"] for event in client["events"]]
    assert len(events) == 1
    assert events[0]["name"] == "create"
    assert events[0]["captured"]["tab"]
