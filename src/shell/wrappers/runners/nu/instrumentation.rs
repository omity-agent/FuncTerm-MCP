use super::capture_state;
use crate::contract::POSIX_COMMAND_FUNCTION;
use crate::shell::wrappers::{VariableNamespace, template, variables};
const EXIT_REQUEST_FILE: &str = "exit-request.nuon";
const EXIT_HOOK: &str = "def exit [@VAR_exit_code@: int = 0, --abort] {
    let @VAR_exit_file@ = ($env.@COMMAND_DIR_ENV@ | path join '@NUSHELL_EXIT_REQUEST@')
    { exit_code: $@VAR_exit_code@, abort: $abort }
        | to nuon
        | save --force --raw $@VAR_exit_file@
    %exit $@VAR_exit_code@ --abort=$abort
}";
pub(super) fn render(template_text: &str) -> String {
    let runner = render_functions(template_text);
    let state_functions = render_functions(capture_state::FUNCTIONS);
    let wrapper = format!(
        "{runner}\n{state_functions}\n{}",
        template::nushell_dispatcher()
    );
    VariableNamespace::new().render(&wrapper)
}
pub(super) fn render_worker(template_text: &str) -> String {
    VariableNamespace::new().render(&render_functions(template_text))
}
fn render_functions(template_text: &str) -> String {
    let exit_hook = template_text.replace("@NUSHELL_EXIT_HOOK@", EXIT_HOOK);
    let exit_signal = exit_hook.replace("@NUSHELL_EXIT_REQUEST@", EXIT_REQUEST_FILE);
    let stateful = exit_signal.replace("@NUSHELL_STATE_FUNCTIONS@", capture_state::FUNCTIONS);
    let protected = stateful.replace(
        "@NUSHELL_PROTECTED_ENVIRONMENT@",
        &variables::nushell_protected_environment_names(),
    );
    template::render_command_function(&protected, POSIX_COMMAND_FUNCTION)
}
