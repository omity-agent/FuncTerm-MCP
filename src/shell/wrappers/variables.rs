use itertools::Itertools as _;
mod namespace;
mod restoration;
pub(in crate::shell) use namespace::VariableNamespace;
pub(in crate::shell) use restoration::startup_environment;
pub(super) fn posix_environment_snapshot() -> String {
    let protected = crate::shell::shims::PROTECTED_ENVIRONMENT_NAMES
        .iter()
        .format_with("\n", |name, format| {
            format(&format_args!(
                "    local @VAR_protected_{name}@=\"${{{name}-}}\""
            ))
        });
    format!(
        "    local @VAR_complete_environment@=\"$(export -p | sed 's/^declare -x /export /')\"\n{protected}"
    )
}
pub(super) fn posix_environment_restore() -> String {
    let protected = crate::shell::shims::PROTECTED_ENVIRONMENT_NAMES
        .iter()
        .format_with("\n", |name, format| {
            format(&format_args!(
                "    export {name}=\"$@VAR_protected_{name}@\""
            ))
        });
    format!(
        "    if [ -z \"${{PATH+x}}\" ] && [ -z \"${{PWD+x}}\" ]; then\n        eval \"$@VAR_complete_environment@\"\n    fi\n{protected}\n    functerm_ensure_shim_path || return 1"
    )
}
pub(super) fn nushell_protected_environment_names() -> String {
    protected_environment_names().join(" ")
}
pub(super) fn cmd_environment_restore() -> String {
    let cleared = protected_environment_names().format_with("\n", |name, format| {
        format(&format_args!("set \"{name}=\""))
    });
    format!(
        "{cleared}\nfor /f \"usebackq delims=\" %%e in (\"%~dp0@VAR_protected_environment_file@.txt\") do set \"%%e\""
    )
}
pub(super) fn cmd_environment_capture() -> String {
    let patterns = protected_environment_names()
        .format_with(" ", |name, format| format(&format_args!("/c:\"{name}=\"")));
    format!(
        "findstr.exe /b /l {patterns} \"%~dp0@VAR_environment_before_file@.txt\" > \"%~dp0@VAR_protected_environment_file@.txt\""
    )
}
pub(super) fn powershell_protected_environment_names() -> String {
    protected_environment_names()
        .format_with(", ", |name, format| format(&format_args!("'{name}'")))
        .to_string()
}
pub(in crate::shell) fn quoted_protected_environment_names() -> String {
    protected_environment_names()
        .format_with(", ", |name, format| format(&format_args!("\"{name}\"")))
        .to_string()
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
