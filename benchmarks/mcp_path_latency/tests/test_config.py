from pathlib import Path

from functerm_mcp_path_latency.config import BenchConfig, resolve_config_paths


def test_resolve_config_paths_resolves_current_device_paths() -> None:
    config = BenchConfig.model_validate(
        {
            "clients": {"list_tools_on_connect": True},
            "server": {"command": "../target/app.exe", "args": ["mcp"], "cwd": ".."},
            "report": {"directory": "reports", "summary_file": "a.json", "events_file": "b.jsonl"},
            "profiles": [{"name": "one", "scenario": [{"name": "x", "tool": "y"}]}],
        }
    )
    resolved = resolve_config_paths(config, Path("C:/repo/bench").resolve())
    assert isinstance(resolved.server.command, str)
    assert Path(resolved.server.command).is_absolute()
    assert resolved.server.cwd is not None
    assert resolved.server.cwd.is_absolute()
    assert resolved.report.directory.is_absolute()


def test_profile_order_rotates_for_each_client() -> None:
    config = BenchConfig.model_validate(
        {
            "clients": {"list_tools_on_connect": True},
            "server": {"command": "functerm", "args": ["mcp"]},
            "report": {"directory": "reports", "summary_file": "a.json", "events_file": "b.jsonl"},
            "profiles": [
                {"name": "one", "scenario": [{"name": "x", "tool": "new_tab"}]},
                {"name": "two", "scenario": [{"name": "y", "tool": "view"}]},
                {"name": "three", "scenario": [{"name": "z", "tool": "view"}]},
            ],
        }
    )
    assert config.client_count() == 3
    assert config.clients.loop_count == 1
    assert [profile.name for profile in config.profile_order_for(0)] == ["one", "two", "three"]
    assert [profile.name for profile in config.profile_order_for(1)] == ["two", "three", "one"]
