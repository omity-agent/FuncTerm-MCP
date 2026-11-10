use super::super::variables;
use super::PosixDialect;
use crate::contract::{
    CLAIM_RECEIPT_FILE, COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV,
    SESSION_COMMANDS_DIRECTORY, SESSION_STATE_DIRECTORY, STDERR_FILE, STDOUT_FILE,
};
pub(in crate::shell) fn script(zsh: bool) -> String {
    let dialect = if zsh {
        PosixDialect::Zsh
    } else {
        PosixDialect::Bash
    };
    format!(
        r#"functerm_prepare_command() {{
    @VAR_claim_error@="$("$@VAR_helper_path@" internal-claim-command --session-root "$FUNCTERM_SESSION_ROOT" --shell "$FUNCTERM_CURRENT_SHELL" --output-file "$FUNCTERM_SESSION_ROOT/{state}/{receipt}" < /dev/null 2>&1)" || {{
        builtin printf '%s\n' "$@VAR_claim_error@" >&2
        return 1
    }}
    {{
        IFS= read -r -d '' @VAR_command_id@ &&
        IFS= read -r -d '' @VAR_native_directory@ &&
        IFS= read -r -d '' @VAR_working_directory@ &&
        IFS= read -r -d '' @VAR_separate_output@
    }} < "$@VAR_receipt_path@" || return 1
    @VAR_directory@="$@VAR_session_root_path@/{commands}/$@VAR_command_id@"
    @VAR_working_directory@="$(functerm_posix_path "$@VAR_working_directory@")" || return 1
    @VAR_stdout_file@="$@VAR_directory@/{stdout}"
    @VAR_stderr_file@="$@VAR_directory@/{stderr}"
    @VAR_previous_command_id@="${{{command_id_env}-}}"
    @VAR_previous_command_directory@="${{{command_dir_env}-}}"
    {previous_flags}
    {snapshot}
    export {command_id_env}="$@VAR_command_id@"
    export {command_dir_env}="$@VAR_directory@"
    functerm_ensure_shim_path || return 1
    @VAR_script@="$(< "$@VAR_directory@/{source}")" || return 1
    {cd} "$@VAR_working_directory@" || return 1
    @VAR_command_started_at@="$(functerm_command_time_millis)" || return 1
    functerm_publish_start "$@VAR_command_id@" "$@VAR_native_directory@"
}}
functerm_finish_command() {{
    {restore}
    if [ "$?" -ne 0 ]; then
        printf '%s\n' 'failed to restore shell shim path' >&2
        @VAR_exit_code@=1
    fi
    @VAR_command_finished_at@="$(functerm_command_time_millis)" || return 1
    @VAR_time_consumption@="$((@VAR_command_finished_at@ - @VAR_command_started_at@))ms"
    functerm_publish_done "$@VAR_command_id@" "$@VAR_exit_code@" "$@VAR_time_consumption@" "$PWD" "$@VAR_native_directory@" || return 1
    functerm_restore_command_environment \
        "$@VAR_had_previous_command_id@" "$@VAR_previous_command_id@" \
        "$@VAR_had_previous_command_directory@" "$@VAR_previous_command_directory@"
    unset @VAR_command_id@ @VAR_native_directory@
    return "$@VAR_exit_code@"
}}
if functerm_prepare_command; then
    if [ "$@VAR_separate_output@" = 1 ]; then
        {{
            functerm_restore_status "$@VAR_previous_status@"
            builtin eval -- "$@VAR_script@"
        }} >| "$@VAR_stdout_file@" 2>| "$@VAR_stderr_file@"
        @VAR_exit_code@=$?
    else
        functerm_restore_status "$@VAR_previous_status@"
        builtin eval -- "$@VAR_script@"
        @VAR_exit_code@=$?
    fi
    functerm_finish_command
fi
"#,
        state = SESSION_STATE_DIRECTORY,
        receipt = CLAIM_RECEIPT_FILE,
        commands = SESSION_COMMANDS_DIRECTORY,
        stdout = STDOUT_FILE,
        stderr = STDERR_FILE,
        source = COMMAND_FILE,
        command_id_env = COMMAND_ID_ENV,
        command_dir_env = COMMAND_DIRECTORY_ENV,
        previous_flags = dialect.previous_flags().replace("local ", ""),
        snapshot = variables::posix_environment_snapshot().replace("local ", ""),
        restore = variables::posix_environment_restore(),
        cd = dialect.cd(),
    )
}
