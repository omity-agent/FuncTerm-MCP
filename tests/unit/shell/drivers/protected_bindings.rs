#[test]
fn shell_environment_lists_preserve_exact_spelling_separators_and_order() {
    let names = super::protected_environment_names().collect::<Vec<_>>();
    assert!(!names.contains(&"PATH"));
    assert_eq!(
        super::nushell_protected_environment_names(),
        names.join(" ")
    );
    assert_eq!(
        super::powershell_protected_environment_names(),
        names
            .iter()
            .map(|name| format!("'{name}'"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    assert_eq!(
        super::quoted_protected_environment_names(),
        names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(", ")
    );
    let cleared = names
        .iter()
        .map(|name| format!("set \"{name}=\""))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        super::cmd_environment_restore(),
        format!(
            "{cleared}\nfor /f \"usebackq delims=\" %%e in (\"%~dp0@VAR_protected_environment_file@.txt\") do set \"%%e\""
        )
    );
    let patterns = names
        .iter()
        .map(|name| format!("/c:\"{name}=\""))
        .collect::<Vec<_>>()
        .join(" ");
    assert_eq!(
        super::cmd_environment_capture(),
        format!(
            "findstr.exe /b /l {patterns} \"%~dp0@VAR_environment_before_file@.txt\" > \"%~dp0@VAR_protected_environment_file@.txt\""
        )
    );
}
#[test]
fn posix_environment_scripts_preserve_exact_rendered_content() {
    let names = crate::shell::shims::PROTECTED_ENVIRONMENT_NAMES;
    let captured = names
        .iter()
        .map(|name| format!("    local @VAR_protected_{name}@=\"${{{name}-}}\""))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        super::posix_environment_snapshot(),
        format!(
            "    local @VAR_complete_environment@=\"$(export -p | sed 's/^declare -x /export /')\"\n{captured}"
        )
    );
    let restored = names
        .iter()
        .map(|name| format!("    export {name}=\"$@VAR_protected_{name}@\""))
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(
        super::posix_environment_restore(),
        format!(
            "    if [ -z \"${{PATH+x}}\" ] && [ -z \"${{PWD+x}}\" ]; then\n        eval \"$@VAR_complete_environment@\"\n    fi\n{restored}\n    functerm_ensure_shim_path || return 1"
        )
    );
}
