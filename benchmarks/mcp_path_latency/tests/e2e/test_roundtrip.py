import json
import sys
from collections.abc import Callable
from pathlib import Path
from subprocess import CompletedProcess


def test_concurrent_profiles_execute_real_commands_and_publish_consistent_reports(
    benchmark_cli: Callable[[list[dict[str, object]]], tuple[CompletedProcess[str], Path]],
) -> None:
    profiles: list[dict[str, object]] = []
    for name in ["alpha", "beta"]:
        profiles.append(
            {
                "name": name,
                "scenario": [
                    {
                        "name": "create",
                        "tool": "new_tab",
                        "arguments": {
                            "exec": [
                                {
                                    "starting_shell": "powershell"
                                    if sys.platform == "win32"
                                    else "bash"
                                }
                            ]
                        },
                        "capture": {"tab": "TAB_ID"},
                    },
                    {
                        "name": "execute",
                        "tool": "send_command",
                        "arguments": {
                            "exec": [
                                {
                                    "tab_id": "${tab}",
                                    "command": "echo ${profile.name}_${client.name}",
                                }
                            ],
                            "wait_timeout": 5,
                        },
                        "capture": {"command": "COMMAND_ID", "output": "STDOUT"},
                    },
                    {
                        "name": "inspect",
                        "tool": "view",
                        "arguments": {"ids": ["${command}"], "wait_timeout": 5},
                        "capture": {"output": "STDOUT", "finished": "FINISHED"},
                    },
                ],
            }
        )
    result, reports = benchmark_cli(profiles)
    summary = json.loads((reports / "summary.json").read_text(encoding="utf-8"))
    assert result.returncode == 0, (result.stdout, result.stderr, summary)
    events = [
        json.loads(line)
        for line in (reports / "events.jsonl").read_text(encoding="utf-8").splitlines()
    ]
    assert summary["succeeded_clients"] == 2
    assert summary["failed_clients"] == 0
    assert [client["profile_order"] for client in summary["clients"]] == [
        ["alpha", "beta"],
        ["beta", "alpha"],
    ]
    assert events == [event for client in summary["clients"] for event in client["events"]]
    assert len(events) == 12
    assert len({event["captured"]["tab"] for event in events if event["name"] == "create"}) == 4
    for start in range(0, len(events), 3):
        created, executed, inspected = events[start : start + 3]
        expected = f"{executed['profile_name']}_client-{executed['client_index'] + 1}"
        assert executed["arguments"]["exec"][0]["tab_id"] == created["captured"]["tab"]
        assert inspected["arguments"]["ids"] == [executed["captured"]["command"]]
        assert expected in executed["captured"]["output"]
        assert inspected["captured"]["output"] == executed["captured"]["output"]
        assert inspected["captured"]["finished"] == "true"
        assert all(event["elapsed_seconds"] > 0 for event in [created, executed, inspected])
