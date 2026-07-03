from pathlib import Path

from functerm_mcp_path_latency.config import BenchConfig, resolve_config_paths


def test_resolve_config_paths_resolves_current_device_paths() -> None:
    config = BenchConfig.model_validate(
        {
            "clients": {"count": 1, "list_tools_on_connect": True},
            "server": {"command": "../target/app.exe", "args": ["mcp"], "cwd": ".."},
            "scenario": [{"name": "x", "tool": "y"}],
            "report": {"directory": "reports", "summary_file": "a.json", "events_file": "b.jsonl"},
        }
    )
    resolved = resolve_config_paths(config, Path("C:/repo/bench").resolve())
    assert isinstance(resolved.server.command, str)
    assert Path(resolved.server.command).is_absolute()
    assert resolved.server.cwd is not None
    assert resolved.server.cwd.is_absolute()
    assert resolved.report.directory.is_absolute()


def test_client_profiles_define_unique_paths_without_global_scenario() -> None:
    config = BenchConfig.model_validate(
        {
            "clients": {
                "list_tools_on_connect": True,
                "profiles": [
                    {"name": "one", "scenario": [{"name": "x", "tool": "new_tab"}]},
                    {"name": "two", "scenario": [{"name": "y", "tool": "view"}]},
                ],
            },
            "server": {"command": "functerm", "args": ["mcp"]},
            "report": {"directory": "reports", "summary_file": "a.json", "events_file": "b.jsonl"},
        }
    )
    assert config.clients.resolved_count() == 2
    assert config.clients.loop_count == 1
    assert config.clients.profile_for(0) is not None
