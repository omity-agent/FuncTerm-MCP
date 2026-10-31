use super::{DriverStartup, StartupContext, os_strings_lower};
use crate::shell::quote;
use crate::shell::shims::CURRENT_SHELL_ENV;
use crate::shell::wrappers::{cmd_dispatcher, cmd_wrapper, startup_environment};
use anyhow::Result;
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let runner = context.startup_directory.join("cmd_run.bat");
    let dispatcher = context.startup_directory.join("f.cmd");
    let init = context.startup_directory.join("cmd_init.bat");
    fs_err::write(&runner, cmd_wrapper())?;
    fs_err::write(&dispatcher, cmd_dispatcher())?;
    fs_err::write(&init, initialization_script(context)?)?;
    let mut args = Vec::new();
    if !context.load_profile {
        args.push("/D".to_owned());
    }
    args.extend([
        "/Q".to_owned(),
        "/K".to_owned(),
        format!("call {}", quote::native_path(&init)?),
    ]);
    Ok(DriverStartup {
        args,
        env: Vec::new(),
    })
}
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    let Some(values) = os_strings_lower(arguments) else {
        return false;
    };
    values
        .iter()
        .all(|value| matches!(value.as_str(), "/d" | "/q" | "/k"))
}
pub(super) fn encode_interactive_line(line: &str) -> String {
    let command = line.trim_start().trim_start_matches('@');
    let for_command = command
        .split_ascii_whitespace()
        .next()
        .is_some_and(|word| word.eq_ignore_ascii_case("for"));
    let binding = if for_command {
        loop_binding(command)
    } else {
        None
    };
    let mut normalized = String::with_capacity(line.len());
    let mut remaining = line;
    while let Some((prefix, tail)) = remaining.split_once('%') {
        normalized.push_str(prefix);
        if binding.is_some()
            && let Some(doubled_tail) = tail.strip_prefix('%')
            && doubled_tail
                .chars()
                .next()
                .is_some_and(|value| value.is_ascii_alphabetic())
        {
            normalized.push_str("%%");
            remaining = doubled_tail;
            continue;
        }
        let variable = tail.chars().next();
        let for_variable = binding.is_some() && variable == binding;
        let positional =
            variable.is_some_and(|value| value.is_ascii_digit() || matches!(value, '*' | '~'));
        if !for_variable
            && !positional
            && let Some((name, after_variable)) = tail.split_once('%')
            && !name.is_empty()
        {
            normalized.push('%');
            normalized.push_str(name);
            normalized.push('%');
            remaining = after_variable;
        } else {
            normalized.push_str("%%");
            remaining = tail;
        }
    }
    normalized.push_str(remaining);
    normalized
}
fn loop_binding(line: &str) -> Option<char> {
    let mut candidate = None;
    for word in line.split_ascii_whitespace().skip(1) {
        if word.eq_ignore_ascii_case("in") || word.to_ascii_lowercase().starts_with("in(") {
            return candidate;
        }
        candidate = match word.as_bytes() {
            &[b'%', name] | &[b'%', b'%', name] if name.is_ascii_alphabetic() => {
                Some(char::from(name))
            }
            _ => None,
        };
    }
    None
}
fn initialization_script(context: StartupContext<'_>) -> Result<String> {
    let style = if context.ignore_shell_style {
        "set \"NO_COLOR=1\"\r\nprompt $G\r\n"
    } else {
        ""
    };
    Ok(format!(
        "@echo off\r\nchcp 65001 >nul\r\n{}\r\nset {CURRENT_SHELL_ENV}=cmd\r\n{}\r\nset \"PATH=%~dp0;%FUNCTERM_SHIM_DIR%;%PATH%\"\r\ndoskey /listsize=0 >nul 2>nul\r\ndoskey f=\r\nif not defined PROMPT prompt $P$G\r\nprompt $E]9999;FuncTerm;prompt;cmd;$P$E\\%PROMPT%\r\ncd /d {}\r\ntype nul > {}\r\n",
        startup_environment(crate::shell::ShellChoice::Cmd, context.environment)?
            .replace('\n', "\r\n"),
        style,
        quote::cmd_string(&quote::native_path(context.cwd)?.replace('%', "%%")),
        quote::cmd_string(&quote::native_path(context.ready_file)?)
    ))
}
