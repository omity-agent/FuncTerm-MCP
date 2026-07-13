from __future__ import annotations

import re
from typing import Any

TOKEN_PATTERN = re.compile(r"\$\{([A-Za-z0-9_.-]+)\}")


def render_value(value: Any, variables: dict[str, str]) -> Any:
    if isinstance(value, str):
        return render_text(value, variables)
    if isinstance(value, list):
        return [render_value(item, variables) for item in value]
    if isinstance(value, dict):
        return {key: render_value(item, variables) for key, item in value.items()}
    return value


def render_text(text: str, variables: dict[str, str]) -> str:

    def replace(match: re.Match[str]) -> str:
        name = match.group(1)
        try:
            return variables[name]
        except KeyError as error:
            raise KeyError(f"missing template variable: {name}") from error

    return TOKEN_PATTERN.sub(replace, text)
