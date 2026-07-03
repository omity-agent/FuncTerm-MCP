from functerm_mcp_path_latency.template import render_value


def test_render_value_handles_nested_values() -> None:
    rendered = render_value(
        {"command": "echo ${client.index}", "items": ["${name}", 3]},
        {"client.index": "7", "name": "FuncTerm"},
    )
    assert rendered == {"command": "echo 7", "items": ["FuncTerm", 3]}
