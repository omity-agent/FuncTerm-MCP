from __future__ import annotations

import argparse
import asyncio
from pathlib import Path

from functerm_mcp_path_latency.config import load_config
from functerm_mcp_path_latency.report import write_reports
from functerm_mcp_path_latency.scenario import run_benchmark


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--config", type=Path, default=Path("settings.yaml"), help="benchmark configuration file"
    )
    args = parser.parse_args()
    config = load_config(args.config.resolve())
    result = asyncio.run(run_benchmark(config))
    summary_path, events_path = write_reports(result, config.report)
    print(f"total_seconds={result.elapsed_seconds:.6f}")
    print(f"succeeded_clients={sum(client.succeeded for client in result.clients)}")
    print(f"failed_clients={sum(not client.succeeded for client in result.clients)}")
    print(f"summary={summary_path}")
    print(f"events={events_path}")
    if not result.succeeded:
        raise SystemExit(1)


if __name__ == "__main__":
    main()
