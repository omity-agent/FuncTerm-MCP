from typing import Any

from pydantic import BaseModel

from functerm_mcp_path_latency.config import StepConfig
from functerm_mcp_path_latency.scenario import run_step


class Content(BaseModel):
    text: str


class Result(BaseModel):
    content: list[Content]


class FakeSession:
    def __init__(self) -> None:
        self.arguments: dict[str, Any] | None = None

    async def call_tool(self, name: str, arguments: dict[str, Any]) -> object:
        self.arguments = arguments
        assert name == "new_tab"
        return Result(content=[Content(text="<TAB_ID>\ntab_1\n</TAB_ID>")])


async def test_run_step_renders_arguments_and_captures_output() -> None:
    session = FakeSession()
    step = StepConfig(
        name="create",
        tool="new_tab",
        arguments={"starting_shell": "${shell}"},
        capture={"tab_id": "TAB_ID"},
    )
    event, captures = await run_step(0, 3, step, session, {"shell": "powershell"})
    assert session.arguments == {"starting_shell": "powershell"}
    assert captures == {"tab_id": "tab_1"}
    assert event.loop_index == 3
    assert event.captured == captures
