from __future__ import annotations

from dataclasses import dataclass
from datetime import UTC, datetime
from statistics import fmean
from typing import Any


@dataclass(frozen=True)
class StepEvent:
    client_index: int
    loop_index: int
    name: str
    tool: str
    elapsed_seconds: float
    arguments: dict[str, Any]
    captured: dict[str, str]

    def to_json(self) -> dict[str, Any]:
        return {
            "client_index": self.client_index,
            "loop_index": self.loop_index,
            "name": self.name,
            "tool": self.tool,
            "elapsed_seconds": self.elapsed_seconds,
            "arguments": self.arguments,
            "captured": self.captured,
        }


@dataclass(frozen=True)
class ClientResult:
    index: int
    profile_name: str | None
    elapsed_seconds: float
    events: list[StepEvent]
    error: str | None

    @property
    def succeeded(self) -> bool:
        return self.error is None

    def to_json(self) -> dict[str, Any]:
        return {
            "index": self.index,
            "profile_name": self.profile_name,
            "elapsed_seconds": self.elapsed_seconds,
            "succeeded": self.succeeded,
            "error": self.error,
            "events": [event.to_json() for event in self.events],
        }


@dataclass(frozen=True)
class BenchmarkResult:
    elapsed_seconds: float
    clients: list[ClientResult]
    started_at: datetime

    @property
    def succeeded(self) -> bool:
        return all(client.succeeded for client in self.clients)

    def summary(self) -> dict[str, Any]:
        elapsed = [client.elapsed_seconds for client in self.clients]
        succeeded = [client for client in self.clients if client.succeeded]
        return {
            "started_at": self.started_at.astimezone(UTC).isoformat(),
            "elapsed_seconds": self.elapsed_seconds,
            "client_count": len(self.clients),
            "succeeded_clients": len(succeeded),
            "failed_clients": len(self.clients) - len(succeeded),
            "client_elapsed_seconds": summarize_seconds(elapsed),
            "clients": [client.to_json() for client in self.clients],
        }


def summarize_seconds(values: list[float]) -> dict[str, float]:
    if not values:
        raise ValueError("values cannot be empty")
    ordered = sorted(values)
    p95_index = min(len(ordered) - 1, int((len(ordered) - 1) * 0.95))
    return {
        "min": ordered[0],
        "mean": fmean(ordered),
        "p95": ordered[p95_index],
        "max": ordered[-1],
    }
