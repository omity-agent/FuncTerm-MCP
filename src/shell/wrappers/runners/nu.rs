use super::super::{template, variables};
use crate::contract::{DISPATCH_FILE, NUSHELL_SUBMISSION_FILE, SESSION_STATE_DIRECTORY};
pub(in crate::shell) fn wrapper() -> String {
    let protected = TEMPLATE
        .replace(
            "@NUSHELL_PROTECTED_ENVIRONMENT@",
            &variables::nushell_protected_environment_names(),
        )
        .replace("@DISPATCH@", DISPATCH_FILE)
        .replace("@SUBMISSION@", NUSHELL_SUBMISSION_FILE)
        .replace("@STATE_DIRECTORY@", SESSION_STATE_DIRECTORY);
    template::render_command_function(&protected, "functerm_finish")
}
const TEMPLATE: &str = "def --wrapped functerm_helper [...@VAR_arguments@] {
    let @VAR_result@ = (^$@VAR_helper_path@ ...$@VAR_arguments@ | complete)
    if $@VAR_result@.exit_code != 0 {
        error make {msg: $@VAR_result@.stderr}
    }
    if ($@VAR_result@.stderr | is-not-empty) { print --stderr $@VAR_result@.stderr }
    $@VAR_result@.stdout
}
def --env functerm_begin [] {
    let @VAR_dispatch@ = $env.FUNCTERM_SESSION_ROOT | path join '@STATE_DIRECTORY@' '@DISPATCH@'
    if not ($@VAR_dispatch@ | path exists) { return }
    let @VAR_receipt@ = functerm_helper internal-claim-command --session-root $env.FUNCTERM_SESSION_ROOT --shell nu | from json
    let @VAR_directory@ = $@VAR_receipt@.directory | path expand
    {
        id: $@VAR_receipt@.command_id, directory: $@VAR_directory@,
        previous_id: $env.@COMMAND_ID_ENV@?, previous_directory: $env.@COMMAND_DIR_ENV@?,
        started: (date now)
    } | to nuon | save --force --raw $@VAR_active_command@
    $env.@COMMAND_ID_ENV@ = $@VAR_receipt@.command_id
    $env.@COMMAND_DIR_ENV@ = $@VAR_directory@
    ensure_nushell_shims
    cd $@VAR_receipt@.working_directory
    functerm_helper internal-write-start --command-id $@VAR_receipt@.command_id --directory $@VAR_directory@ | ignore
    cp --force ($@VAR_directory@ | path join '@COMMAND@') ($@VAR_directory@ | path join '@SUBMISSION@')
}
def --env functerm_finish [@VAR_requested_exit@: int = -1] {
    let @VAR_status@ = $env.LAST_EXIT_CODE? | default 0
    let @VAR_state_file@ = $@VAR_active_command@
    if not ($@VAR_state_file@ | path exists) { return }
    let @VAR_state@ = open --raw $@VAR_state_file@ | from nuon
    let @VAR_exit_code@ = if $@VAR_requested_exit@ >= 0 { $@VAR_requested_exit@ } else { $@VAR_status@ }
    let @VAR_elapsed@ = ((date now) - $@VAR_state@.started) | into string
    let @VAR_cwd@ = $env.PWD
    if ($env.FUNCTERM_SESSION_ROOT? | is-empty) {
        load-env ($@VAR_recovery_environment@ | reject --optional PWD FILE_PWD CURRENT_FILE config)
        $env.config = $@VAR_recovery_environment@.config
        $env.config.hooks.pre_execution = ($env.config.hooks.pre_execution | append { functerm_begin })
        $env.config.hooks.pre_prompt = ($env.config.hooks.pre_prompt | append { functerm_finish })
    } else {
        load-env ($@VAR_recovery_environment@ | select --optional @NUSHELL_PROTECTED_ENVIRONMENT@)
    }
    rm $@VAR_state_file@
    if ($@VAR_state@.previous_id | is-empty) {
        hide-env --ignore-errors @COMMAND_ID_ENV@
    } else {
        $env.@COMMAND_ID_ENV@ = $@VAR_state@.previous_id
    }
    if ($@VAR_state@.previous_directory | is-empty) {
        hide-env --ignore-errors @COMMAND_DIR_ENV@
    } else {
        $env.@COMMAND_DIR_ENV@ = $@VAR_state@.previous_directory
    }
    ensure_nushell_shims
    functerm_helper internal-write-done --command-id $@VAR_state@.id --exit-code $@VAR_exit_code@ --time-consumption $@VAR_elapsed@ --cwd $@VAR_cwd@ --directory $@VAR_state@.directory | ignore
}
def --env ensure_nushell_shims [] {
    let @VAR_shim@ = $env.FUNCTERM_SHIM_DIR
    $env.PATH = ($env.PATH | where {|@VAR_entry@| $@VAR_entry@ != $@VAR_shim@ } | prepend $@VAR_shim@)
}
def exit [@VAR_code@: int = 0, --abort] {
    functerm_finish $@VAR_code@
    %exit $@VAR_code@ --abort=$abort
}
";
