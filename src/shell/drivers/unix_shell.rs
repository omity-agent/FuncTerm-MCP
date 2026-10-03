use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{VariableNamespace, bash_wrapper, startup_environment, zsh_wrapper};
use anyhow::{Context as _, Result};
pub(super) fn interactive_arguments(
    choice: crate::shell::ShellChoice,
    arguments: &[std::ffi::OsString],
) -> bool {
    let Some(values) = os_strings_lower(arguments) else {
        return false;
    };
    values.iter().all(|value| {
        matches!(value.as_str(), "-i" | "-l" | "--login")
            || (choice == crate::shell::ShellChoice::Bash
                && matches!(value.as_str(), "--noprofile" | "--norc"))
            || (choice == crate::shell::ShellChoice::Zsh
                && matches!(value.as_str(), "--no-rcs" | "-f"))
    })
}
pub(super) fn bash_startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init_path = context.startup_directory.join("bash_init.sh");
    let profile = if context.load_profile {
        "if builtin test -f \"$HOME/.bashrc\"; then\n    builtin source \"$HOME/.bashrc\"\nfi\n"
    } else {
        ""
    };
    let guard = r#"if [[ $- == *e* ]]; then
    printf '%s\n' 'FuncTerm: disabling errexit to preserve command completion reporting' >&2
    builtin set +o errexit
fi
@VAR_aliases_enabled@=0
if builtin shopt -q expand_aliases; then @VAR_aliases_enabled@=1; fi
builtin shopt -u expand_aliases
builtin bind '"\C-j": accept-line'
"#;
    let restore = "if builtin test \"$@VAR_aliases_enabled@\" = 1; then builtin shopt -s expand_aliases; fi\nbuiltin unset @VAR_aliases_enabled@\n";
    let initialization = initialization_script(context, "bash", &bash_wrapper(), ">", restore)?;
    let script = format!("{profile}{guard}{DISPATCH_ALIAS_GUARD}{initialization}");
    fs_err::write(&init_path, VariableNamespace::new().render(&script))?;
    Ok(DriverStartup {
        args: vec![
            "--noprofile".to_owned(),
            "--rcfile".to_owned(),
            quote::native_path(&init_path)?,
            "-i".to_owned(),
        ],
        env: Vec::new(),
    })
}
pub(super) fn zsh_startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let init_path = context.startup_directory.join(".zshrc");
    let namespace = VariableNamespace::new();
    let original_zdotdir = context.environment.value("ZDOTDIR");
    let original = original_zdotdir
        .as_deref()
        .map(|value| {
            value
                .to_str()
                .context("ZDOTDIR is not valid UTF-8")
                .map(|text| format!("builtin export ZDOTDIR={}\n", quote::posix_string(text)))
        })
        .transpose()?
        .unwrap_or_else(|| "builtin unset ZDOTDIR\n".to_owned());
    let env_profile = if context.load_profile {
        "if builtin test -f \"${ZDOTDIR-$HOME}/.zshenv\"; then\n    builtin source \"${ZDOTDIR-$HOME}/.zshenv\"\nfi\n"
    } else {
        "builtin unsetopt globalrcs\n"
    };
    let env_script = format!(
        "{original}{env_profile}builtin typeset -g @VAR_user_zdotdir@=\"${{ZDOTDIR-}}\"\nbuiltin typeset -g @VAR_zdotdir_set@=\"${{ZDOTDIR+x}}\"\nbuiltin export ZDOTDIR={}\nbuiltin setopt rcs\n",
        quote::posix_string(&quote::native_path(context.startup_directory)?),
    );
    fs_err::write(
        context.startup_directory.join(".zshenv"),
        namespace.render(&env_script),
    )?;
    let rc_profile = if context.load_profile {
        "if builtin test -f \"${ZDOTDIR-$HOME}/.zshrc\"; then\n    builtin source \"${ZDOTDIR-$HOME}/.zshrc\"\nfi\n"
    } else {
        ""
    };
    let guard = format!(
        "if builtin test \"$@VAR_zdotdir_set@\" = x; then\n    builtin export ZDOTDIR=\"$@VAR_user_zdotdir@\"\nelse\n    builtin unset ZDOTDIR\nfi\nbuiltin unset @VAR_user_zdotdir@ @VAR_zdotdir_set@\n{rc_profile}builtin typeset @VAR_aliases_enabled@=0\nif [[ -o aliases ]]; then @VAR_aliases_enabled@=1; fi\nbuiltin setopt noaliases\nbuiltin bindkey '^J' accept-line\n"
    );
    let restore = "if builtin test \"$@VAR_aliases_enabled@\" = 1; then builtin setopt aliases; fi\nbuiltin unset @VAR_aliases_enabled@\n";
    let initialization = initialization_script(context, "zsh", &zsh_wrapper(), ">|", restore)?;
    fs_err::write(
        &init_path,
        namespace.render(&format!("{guard}{DISPATCH_ALIAS_GUARD}{initialization}")),
    )?;
    Ok(DriverStartup {
        args: vec!["-i".to_owned()],
        env: vec![(
            "ZDOTDIR".to_owned(),
            quote::native_path(context.startup_directory)?,
        )],
    })
}
const DISPATCH_ALIAS_GUARD: &str =
    "if builtin alias f > /dev/null 2>&1; then\n    builtin unalias f\nfi\n";
fn initialization_script(
    context: StartupContext<'_>,
    shell: &str,
    wrapper: &str,
    overwrite: &str,
    restore_options: &str,
) -> Result<String> {
    let choice = if shell == "bash" {
        crate::shell::ShellChoice::Bash
    } else {
        crate::shell::ShellChoice::Zsh
    };
    let initialization = format!(
        "{}\nexport {CURRENT_SHELL_ENV}={shell}\n{wrapper}\n{restore_options}@VAR_cwd@=$(functerm_posix_path {}) || exit 1\n@VAR_ready_file@=$(functerm_posix_path {}) || exit 1\nbuiltin cd \"$@VAR_cwd@\"\n: {overwrite} \"$@VAR_ready_file@\"\n",
        startup_environment(choice, context.environment)?,
        quote::posix_string(&quote::native_path(context.cwd)?),
        quote::posix_string(&quote::native_path(context.ready_file)?)
    );
    Ok(initialization)
}
