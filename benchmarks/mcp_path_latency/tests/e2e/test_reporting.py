import json
import sys
from collections.abc import Callable
from pathlib import Path
from subprocess import CompletedProcess


def test_real_tool_failure_returns_nonzero_and_preserves_completed_profiles(
    benchmark_cli: Callable[[list[dict[str, object]]], tuple[CompletedProcess[str], Path]],
) -> None:
    profiles: list[dict[str, object]] = [
        {
            "name": "completed-profile",
            "scenario": [
                {
                    "name": "create",
                    "tool": "new_tab",
                    "arguments": {
                        "exec": [
                            {"starting_shell": "powershell" if sys.platform == "win32" else "bash"}
                        ]
                    },
                    "capture": {"tab": "TAB_ID"},
                }
            ],
        },
        {
            "name": "failing-profile",
            "scenario": [
                {
                    "name": "invalid-view",
                    "tool": "view",
                    "arguments": {"ids": ["missing-e2e-command"], "wait_timeout": 0},
                }
            ],
        },
    ]
    result, reports = benchmark_cli(profiles)
    assert result.returncode == 1, result.stdout + result.stderr
    summary = json.loads((reports / "summary.json").read_text(encoding="utf-8"))
    events = [
        json.loads(line)
        for line in (reports / "events.jsonl").read_text(encoding="utf-8").splitlines()
    ]
    assert summary["failed_clients"] == 2
    assert summary["succeeded_clients"] == 0
    for client in summary["clients"]:
        assert "missing-e2e-command" in client["error"]
        assert not client["succeeded"]
    assert events == summary["clients"][0]["events"]
    assert summary["clients"][1]["events"] == []
    assert len(events) == 1
    assert events[0]["name"] == "create"
    assert events[0]["captured"]["tab"]
