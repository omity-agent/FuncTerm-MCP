from functerm_mcp_path_latency.extraction import extract_tag


def test_extract_tag_keeps_inner_unescaped_text() -> None:
    text = "<STDOUT>\nhello <raw> & not escaped\n</STDOUT>"
    assert extract_tag(text, "STDOUT") == "hello <raw> & not escaped"
