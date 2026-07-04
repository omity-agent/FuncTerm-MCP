from __future__ import annotations

import re


def extract_tag(text: str, tag: str) -> str:
    pattern = re.compile(
        rf"<{re.escape(tag)}>\s*(.*?)\s*</{re.escape(tag)}>", re.DOTALL
    )
    match = pattern.search(text)
    if match is None:
        raise ValueError(f"tag {tag} was not found in tool output")
    return match.group(1)
