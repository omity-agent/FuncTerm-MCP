from __future__ import annotations

from collections.abc import AsyncIterator
from contextlib import asynccontextmanager

from mcp import ClientSession, StdioServerParameters
from mcp.client.stdio import stdio_client

from functerm_mcp_path_latency.config import ServerConfig


@asynccontextmanager
async def open_mcp_session(
    server: ServerConfig, *, list_tools_on_connect: bool
) -> AsyncIterator[ClientSession]:
    cwd = None if server.cwd is None else str(server.cwd)
    parameters = StdioServerParameters(
        command=server.selected_command(), args=server.args, cwd=cwd, env=server.env
    )
    async with (
        stdio_client(parameters) as (read_stream, write_stream),
        ClientSession(read_stream, write_stream) as session,
    ):
        await session.initialize()
        if list_tools_on_connect:
            await session.list_tools()
        yield session


def tool_result_text(result: object) -> str:
    if bool(getattr(result, "isError", False)):
        raise RuntimeError(str(result))
    content = getattr(result, "content", None)
    if not isinstance(content, list):
        raise TypeError("MCP tool result does not contain a content list")
    parts: list[str] = []
    for item in content:
        text = getattr(item, "text", None)
        if not isinstance(text, str):
            raise TypeError(f"MCP tool result contains non-text content: {type(item).__name__}")
        parts.append(text)
    return "\n".join(parts)
