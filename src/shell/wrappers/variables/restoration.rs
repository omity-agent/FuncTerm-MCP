use crate::runtime::protocol::EnvironmentSnapshot;
use crate::shell::{ShellChoice, quote};
use anyhow::{Context as _, Result, bail, ensure};
pub(in crate::shell) fn startup_environment(
    shell: ShellChoice,
    environment: &EnvironmentSnapshot,
) -> Result<String> {
    let mut script = String::new();
    for name in super::protected_environment_names() {
        let environment_value = environment.value(name);
        let text = environment_value
            .as_deref()
            .map(|value| {
                value
                    .to_str()
                    .with_context(|| format!("non-UTF-8 startup environment value: {name}"))
            })
            .transpose()?;
        let statement = match shell {
            ShellChoice::PowerShell => format!(
                "[Environment]::SetEnvironmentVariable('{name}', {})",
                text.map_or_else(|| "$null".to_owned(), quote::powershell_string),
            ),
            ShellChoice::Bash | ShellChoice::Zsh => text.map_or_else(
                || format!("builtin unset {name}"),
                |value| format!("builtin export {name}={}", quote::posix_string(value)),
            ),
            ShellChoice::NuShell => text.map_or_else(
                || format!("hide-env --ignore-errors {name}"),
                |value| format!("$env.{name} = {}", quote::nushell_string(value)),
            ),
            ShellChoice::Cmd => {
                let content = text.unwrap_or_default();
                ensure!(
                    !content.contains(['\r', '\n', '"']),
                    "invalid CMD startup environment value: {name}"
                );
                format!("set \"{name}={}\"", content.replace('%', "%%"))
            }
            ShellChoice::Python => text.map_or_else(
                || Ok(format!("@VAR_os@.environ.pop({name:?}, None)")),
                |value| {
                    sonic_rs::to_string(value)
                        .map(|encoded| format!("@VAR_os@.environ[{name:?}] = {encoded}"))
                },
            )?,
            ShellChoice::Bun => bail!("Bun has no injected Shell Profile loader"),
        };
        script.push_str(&statement);
        script.push('\n');
    }
    Ok(script)
}
