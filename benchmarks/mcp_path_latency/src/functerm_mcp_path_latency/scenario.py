from __future__ import annotations

import asyncio
import time
from collections.abc import Awaitable, Callable
from datetime import UTC, datetime
from typing import Any, Protocol

from functerm_mcp_path_latency.config import BenchConfig, ServerConfig, StepConfig
from functerm_mcp_path_latency.extraction import extract_tag
from functerm_mcp_path_latency.mcp_session import open_mcp_session, tool_result_text
from functerm_mcp_path_latency.result import BenchmarkResult, ClientResult, StepEvent
from functerm_mcp_path_latency.template import render_value


class ToolSession(Protocol):
    def call_tool(self, name: str, arguments: dict[str, Any]) -> Awaitable[object]: ...


SessionFactory = Callable[[ServerConfig, bool], Any]


async def run_benchmark(config: BenchConfig) -> BenchmarkResult:
    started_at = datetime.now(UTC)
    started = time.perf_counter()
    tasks = [
        asyncio.create_task(run_client(index, config))
        for index in range(config.clients.resolved_count())
    ]
    clients = await asyncio.gather(*tasks)
    return BenchmarkResult(
        elapsed_seconds=time.perf_counter() - started, clients=clients, started_at=started_at
    )


async def run_client(config_index: int, config: BenchConfig) -> ClientResult:
    started = time.perf_counter()
    events: list[StepEvent] = []
    profile = config.clients.profile_for(config_index)
    profile_name = None if profile is None else profile.name
    profile_variables = {} if profile is None else profile.variables
    steps = config.scenario if profile is None or not profile.scenario else profile.scenario
    variables = {
        **profile_variables,
        "client.index": str(config_index),
        "client.number": str(config_index + 1),
        "client.name": f"client-{config_index}" if profile_name is None else profile_name,
    }
    try:
        async with open_mcp_session(
            config.server, list_tools_on_connect=config.clients.list_tools_on_connect
        ) as session:
            for step in steps:
                event, captures = await run_step(config_index, step, session, variables)
                variables.update(captures)
                events.append(event)
        return ClientResult(config_index, profile_name, time.perf_counter() - started, events, None)
    except Exception as error:
        return ClientResult(
            config_index, profile_name, time.perf_counter() - started, events, repr(error)
        )


async def run_step(
    client_index: int, step: StepConfig, session: ToolSession, variables: dict[str, str]
) -> tuple[StepEvent, dict[str, str]]:
    arguments = render_value(step.arguments, variables)
    if not isinstance(arguments, dict):
        raise TypeError("rendered step arguments must be a mapping")
    started = time.perf_counter()
    result = await session.call_tool(step.tool, arguments)
    elapsed = time.perf_counter() - started
    text = tool_result_text(result)
    captures = {name: extract_tag(text, tag) for name, tag in step.capture.items()}
    event = StepEvent(
        client_index=client_index,
        name=step.name,
        tool=step.tool,
        elapsed_seconds=elapsed,
        arguments=arguments,
        captured=captures,
    )
    return event, captures
