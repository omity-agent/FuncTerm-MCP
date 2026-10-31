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
pub(super) fn normalize_batch_line(line: &str) -> String {
    let trimmed = line.trim_start();
    if !trimmed
        .get(..4)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("for "))
    {
        return line.to_owned();
    }
    let characters: Vec<_> = line.chars().collect();
    let mut normalized = String::with_capacity(line.len());
    for (index, &character) in characters.iter().enumerate() {
        let is_for_variable = character == '%'
            && characters
                .get(index + 1)
                .is_some_and(char::is_ascii_alphabetic)
            && (index == 0 || characters.get(index - 1) != Some(&'%'))
            && characters.get(index + 2).is_none_or(|following| {
                following.is_whitespace() || matches!(following, ')' | '(' | ',' | ';')
            });
        if is_for_variable {
            normalized.push('%');
        }
        normalized.push(character);
    }
    normalized
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
