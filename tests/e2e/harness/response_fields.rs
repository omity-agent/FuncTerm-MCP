extern crate alloc;
use alloc::collections::{BTreeMap, BTreeSet};
use std::process::Output;
pub(crate) struct TabCreated {
    pub(crate) tab_id: String,
}
#[derive(Debug)]
pub(crate) struct CommandResult {
    pub(crate) cwd: String,
    pub(crate) finished: bool,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    pub(crate) exit_code: Option<i32>,
    pub(crate) time_consumption: String,
}
pub(crate) struct TabView {
    pub(crate) alive: bool,
    pub(crate) cwd: String,
    pub(crate) screen: String,
}
pub(crate) fn parse_command_result(output: &Output) -> CommandResult {
    let text = checked_stdout(output);
    CommandResult {
        cwd: element(&text, "CWD"),
        finished: element(&text, "FINISHED").parse().unwrap(),
        exit_code: parse_exit_code(&element(&text, "EXIT_CODE")),
        time_consumption: element(&text, "TIME_CONSUMPTION"),
        stdout: element(&text, "STDOUT"),
        stderr: element(&text, "STDERR").trim_end().to_owned(),
    }
}
pub(crate) fn parse_command_id(output: &Output) -> String {
    element(&checked_stdout(output), "COMMAND_ID")
}
pub(crate) fn parse_tab_view(output: &Output) -> TabView {
    let text = checked_stdout(output);
    TabView {
        alive: element(&text, "ALIVE").parse().unwrap(),
        cwd: element(&text, "CWD"),
        screen: element(&text, "SCREEN"),
    }
}
#[cfg(windows)]
pub(crate) fn assert_powershell_primary_prompt(view: &TabView) {
    assert!(
        !view.screen.lines().any(|line| line.trim() == ">>"),
        "PowerShell should not enter a continuation prompt:\n{}",
        view.screen
    );
    let final_line = view.screen.lines().next_back().unwrap_or_default();
    assert!(
        final_line == "PS> " || (final_line.starts_with("PS ") && final_line.ends_with("> ")),
        "PowerShell should finish at its primary prompt:\n{}",
        view.screen
    );
}
pub(crate) fn parse_tab_created(output: &Output) -> TabCreated {
    let text = checked_stdout(output);
    TabCreated {
        tab_id: element(&text, "TAB_ID"),
    }
}
fn checked_stdout(output: &Output) -> String {
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout.clone()).unwrap()
}
fn element(text: &str, name: &str) -> String {
    let content = tagged_content(text, name).unwrap();
    if matches!(name, "STDOUT" | "STDERR") {
        return content.to_owned();
    }
    content
        .strip_prefix('\n')
        .unwrap()
        .trim_end_matches('\n')
        .to_owned()
}
pub(crate) fn tagged_content<'text>(text: &'text str, name: &str) -> Option<&'text str> {
    let tag = text
        .lines()
        .filter_map(|line| line.strip_prefix('<')?.strip_suffix('>'))
        .find(|tag| tag.eq_ignore_ascii_case(name))?;
    let (_, after_open) = text.split_once(&format!("<{tag}>"))?;
    let (content, _) = after_open.rsplit_once(&format!("</{tag}>"))?;
    Some(content)
}
pub(crate) fn assert_tag_randomization<'text>(documents: impl IntoIterator<Item = &'text str>) {
    let mut observed = BTreeMap::<String, BTreeSet<&str>>::new();
    for document in documents {
        let mut pending = Vec::new();
        for tag in document
            .lines()
            .filter_map(|line| line.strip_prefix('<')?.strip_suffix('>'))
        {
            if let Some(name) = tag.strip_prefix('/') {
                assert_eq!(pending.pop(), Some(name), "{document}");
            } else {
                pending.push(tag);
                observed
                    .entry(tag.to_ascii_uppercase())
                    .or_default()
                    .insert(tag);
            }
        }
        assert!(pending.is_empty(), "{document}");
    }
    assert!(!observed.is_empty());
    for (name, variants) in observed {
        assert!(variants.len() > 2, "{name}: {variants:?}");
        for (index, letter) in name
            .bytes()
            .enumerate()
            .filter(|&(_, byte)| byte.is_ascii_alphabetic())
        {
            for expected in [letter.to_ascii_lowercase(), letter] {
                assert!(
                    variants
                        .iter()
                        .any(|tag| tag.as_bytes().get(index) == Some(&expected)),
                    "{name}: {variants:?}"
                );
            }
        }
    }
}
fn parse_exit_code(value: &str) -> Option<i32> {
    match value {
        "" => None,
        code => Some(code.parse().unwrap()),
    }
}
