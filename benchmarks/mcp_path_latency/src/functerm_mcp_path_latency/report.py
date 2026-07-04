from __future__ import annotations

import json
from pathlib import Path

from functerm_mcp_path_latency.config import ReportConfig
from functerm_mcp_path_latency.result import BenchmarkResult


def write_reports(
    result: BenchmarkResult, config: ReportConfig
) -> tuple[Path, Path]:
    config.directory.mkdir(parents=True, exist_ok=True)
    summary_path = config.directory / config.summary_file
    events_path = config.directory / config.events_file
    write_json(summary_path, result.summary())
    with events_path.open("w", encoding="utf-8", newline="\n") as file:
        for client in result.clients:
            for event in client.events:
                file.write(json.dumps(event.to_json(), ensure_ascii=False))
                file.write("\n")
    return summary_path, events_path


def write_json(path: Path, value: object) -> None:
    with path.open("w", encoding="utf-8", newline="\n") as file:
        json.dump(value, file, ensure_ascii=False, indent=2)
        file.write("\n")
