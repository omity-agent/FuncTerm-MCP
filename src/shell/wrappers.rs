mod batch;
mod nu;
mod posix_dialect;
mod posix_function;
mod posix_startup;
mod pwsh;
mod start;
mod template;
mod variables;
pub(super) use batch::wrapper as cmd_wrapper;
pub(super) use nu::wrapper as nushell_wrapper;
pub(super) use posix_startup::{bash_wrapper, zsh_wrapper};
pub(super) use pwsh::wrapper as powershell_wrapper;
pub(super) use template::cmd_dispatcher;
pub(in crate::shell) use variables::{VariableNamespace, quoted_protected_environment_names};
#[cfg(test)]
mod tests {
    use crate::contract::{
        COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV, COMMAND_INPUT_DIRECTORY, COMMAND_OUTPUT_DIRECTORY,
        COMMAND_STATE_DIRECTORY, DONE_FILE, HELPER_EXECUTABLE_ENV, STDERR_FILE, STDOUT_FILE,
    };
    #[test]
    fn every_wrapper_uses_the_command_record_contract() {
        for (name, wrapper) in [
            ("cmd", super::cmd_wrapper()),
            ("nushell", super::nushell_wrapper()),
            ("bash", super::bash_wrapper()),
            ("zsh", super::zsh_wrapper()),
            ("powershell", super::powershell_wrapper()),
        ] {
            for required in [
                COMMAND_INPUT_DIRECTORY,
                COMMAND_OUTPUT_DIRECTORY,
                COMMAND_STATE_DIRECTORY,
                STDOUT_FILE,
                STDERR_FILE,
                DONE_FILE,
                COMMAND_ID_ENV,
                COMMAND_DIRECTORY_ENV,
                HELPER_EXECUTABLE_ENV,
                "internal-ensure-shims",
                "internal-write-start",
                "internal-write-done",
            ] {
                assert!(
                    wrapper.contains(required),
                    "{name} wrapper is missing command contract value {required}"
                );
            }
        }
    }
    #[test]
    fn every_wrapper_defines_the_short_dispatcher() {
        for (name, wrapper) in [
            ("cmd", super::cmd_dispatcher()),
            ("nushell", super::nushell_wrapper()),
            ("bash", super::bash_wrapper()),
            ("zsh", super::zsh_wrapper()),
            ("powershell", super::powershell_wrapper()),
        ] {
            for required in [
                crate::contract::DISPATCH_FILE,
                crate::contract::COMMAND_WORKING_DIRECTORY_FILE,
                crate::contract::SESSION_COMMANDS_DIRECTORY,
            ] {
                assert!(
                    wrapper.contains(required),
                    "{name} dispatcher is missing command contract value {required}"
                );
            }
        }
    }
    #[test]
    fn powershell_wrapper_captures_helper_failures_and_promotes_user_state() {
        let wrapper = super::powershell_wrapper();
        for required in [
            ".RedirectStandardOutput = $true",
            ".RedirectStandardError = $true",
            ".ExitCode",
            "Set-Variable -Scope Global",
            "Function:\\global:",
            "Set-Alias -Scope Global",
        ] {
            assert!(wrapper.contains(required));
        }
    }
    #[test]
    fn every_wrapper_randomizes_control_variable_names() {
        let wrappers = [
            super::cmd_wrapper(),
            super::cmd_dispatcher(),
            super::nushell_wrapper(),
            super::bash_wrapper(),
            super::zsh_wrapper(),
            super::powershell_wrapper(),
        ];
        for wrapper in &wrappers {
            assert!(!wrapper.contains("@VAR_"));
            assert!(has_base36_suffix(wrapper));
        }
        assert_ne!(super::powershell_wrapper(), super::powershell_wrapper());
    }
    #[test]
    fn nushell_wrapper_persists_serializable_state() {
        let wrapper = super::nushell_wrapper();
        for required in [
            "nushell-env.nuon",
            "nushell-config.nuon",
            "nushell-declarations.nu",
            "to nuon",
            "load-env",
            "view source",
        ] {
            assert!(wrapper.contains(required));
        }
    }
    fn has_base36_suffix(wrapper: &str) -> bool {
        wrapper
            .split(|character: char| {
                !character.is_ascii_lowercase() && !character.is_ascii_digit() && character != '_'
            })
            .any(|word| {
                let Some((_, suffix)) = word.rsplit_once('_') else {
                    return false;
                };
                suffix.len() == 12
                    && suffix
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            })
    }
}
