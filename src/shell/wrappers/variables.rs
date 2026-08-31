const SUFFIX_LENGTH: usize = 12;
const BASE36_ALPHABET: [char; 36] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
const MARKER_START: &str = "@VAR_";
pub(in crate::shell) struct VariableNamespace {
    suffix: String,
}
impl VariableNamespace {
    pub(in crate::shell) fn new() -> Self {
        Self {
            suffix: nanoid::nanoid!(SUFFIX_LENGTH, &BASE36_ALPHABET),
        }
    }
    pub(in crate::shell) fn render(&self, template: &str) -> String {
        let mut rendered = String::with_capacity(template.len());
        let mut remaining = template;
        while let Some((before_marker, marker_tail)) = remaining.split_once(MARKER_START) {
            rendered.push_str(before_marker);
            let Some((semantic, after_marker)) = marker_tail.split_once('@') else {
                panic!("wrapper variable marker must end with '@'");
            };
            assert!(
                valid_semantic_prefix(semantic),
                "wrapper variable semantic prefix is invalid: {semantic}"
            );
            rendered.push_str(semantic);
            rendered.push('_');
            rendered.push_str(&self.suffix);
            remaining = after_marker;
        }
        rendered.push_str(remaining);
        rendered
    }
}
fn valid_semantic_prefix(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
pub(super) fn posix_environment_snapshot() -> String {
    let protected = crate::shell::shims::PROTECTED_ENVIRONMENT_NAMES
        .iter()
        .map(|name| format!("    local @VAR_protected_{name}@=\"${{{name}-}}\""))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "    local @VAR_complete_environment@=\"$(export -p | sed 's/^declare -x /export /')\"\n{protected}"
    )
}
pub(super) fn posix_environment_restore() -> String {
    let protected = crate::shell::shims::PROTECTED_ENVIRONMENT_NAMES
        .iter()
        .map(|name| format!("    export {name}=\"$@VAR_protected_{name}@\""))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "    if [ -z \"${{PATH+x}}\" ] && [ -z \"${{PWD+x}}\" ]; then\n        eval \"$@VAR_complete_environment@\"\n    fi\n{protected}"
    )
}
pub(super) fn nushell_protected_environment_names() -> String {
    protected_environment_names().collect::<Vec<_>>().join(" ")
}
pub(super) fn cmd_environment_restore() -> String {
    let cleared = protected_environment_names()
        .map(|name| format!("set \"{name}=\""))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "{cleared}\nfor /f \"usebackq delims=\" %%e in (\"%~dp0@VAR_protected_environment_file@.txt\") do set \"%%e\""
    )
}
pub(super) fn cmd_environment_capture() -> String {
    let patterns = protected_environment_names()
        .map(|name| format!("/c:\"{name}=\""))
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "findstr.exe /b /l {patterns} \"%~dp0@VAR_environment_before_file@.txt\" > \"%~dp0@VAR_protected_environment_file@.txt\""
    )
}
pub(super) fn powershell_protected_environment_names() -> String {
    protected_environment_names()
        .map(|name| format!("'{name}'"))
        .collect::<Vec<_>>()
        .join(", ")
}
pub(in crate::shell) fn quoted_protected_environment_names() -> String {
    protected_environment_names()
        .map(|name| format!("\"{name}\""))
        .collect::<Vec<_>>()
        .join(", ")
}
fn protected_environment_names() -> impl Iterator<Item = &'static str> {
    crate::shell::shims::PROTECTED_ENVIRONMENT_NAMES
        .iter()
        .copied()
        .chain([
            crate::contract::COMMAND_ID_ENV,
            crate::contract::COMMAND_DIRECTORY_ENV,
        ])
}
