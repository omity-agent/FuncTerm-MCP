use super::posix_dialect::PosixDialect;
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, COMMAND_INPUT_DIRECTORY,
    COMMAND_OUTPUT_DIRECTORY, COMMAND_STATE_DIRECTORY, DONE_FILE, HELPER_EXECUTABLE_ENV,
    POSIX_COMMAND_FUNCTION, STDERR_FILE, STDOUT_FILE,
};
pub(super) fn command_function(dialect: PosixDialect) -> String {
    format!(
        r#"{name}() {{
		    local @VAR_command_id@="$1"
	    local @VAR_native_directory@="$2"
	    local @VAR_directory@="$@VAR_native_directory@"
	    local @VAR_working_directory@="$3"
	    @VAR_directory@="$(functerm_posix_path "$@VAR_directory@")" || return 1
	    @VAR_working_directory@="$(functerm_posix_path "$@VAR_working_directory@")" || return 1
	    local @VAR_input_dir@="$@VAR_directory@/{input_dir}"
	    local @VAR_output_dir@="$@VAR_directory@/{output_dir}"
	    local @VAR_state_dir@="$@VAR_directory@/{state_dir}"
	    {mkdir} "$@VAR_input_dir@" "$@VAR_output_dir@" "$@VAR_state_dir@" || return 1
	    local @VAR_stdout_file@="$@VAR_output_dir@/{stdout}"
	    local @VAR_stderr_file@="$@VAR_output_dir@/{stderr}"
	    local @VAR_command_file@="$@VAR_input_dir@/{command_file}"
	    local @VAR_done_file@="$@VAR_state_dir@/{done}"
	    local @VAR_previous_command_id@="${{{command_id_env}-}}"
	    local @VAR_previous_command_directory@="${{{command_dir_env}-}}"
	{previous_flags}
	{environment_snapshot}
	    export {command_id_env}="$@VAR_command_id@"
	    export {command_dir_env}="$@VAR_directory@"
	    if ! functerm_ensure_shims; then
	        local @VAR_publish_result@=0
	        functerm_publish_done "$@VAR_command_id@" 1 "0ns" "$PWD" "$@VAR_native_directory@" || @VAR_publish_result@=$?
	        functerm_restore_command_environment \
	            "$@VAR_had_previous_command_id@" "$@VAR_previous_command_id@" \
	            "$@VAR_had_previous_command_directory@" "$@VAR_previous_command_directory@"
	        if [ "$@VAR_publish_result@" -ne 0 ]; then
	            return "$@VAR_publish_result@"
	        fi
	        return 1
	    fi
	    functerm_prepend_shim_path || return 1
	    local @VAR_script@
	    if ! @VAR_script@="$(cat "$@VAR_command_file@" 2> "$@VAR_stderr_file@")"; then
	        local @VAR_publish_result@=0
	        functerm_publish_done "$@VAR_command_id@" 1 "0ns" "$PWD" "$@VAR_native_directory@" || @VAR_publish_result@=$?
	        cat "$@VAR_stderr_file@" >&2
	        functerm_restore_command_environment \
	            "$@VAR_had_previous_command_id@" "$@VAR_previous_command_id@" \
	            "$@VAR_had_previous_command_directory@" "$@VAR_previous_command_directory@"
	        if [ "$@VAR_publish_result@" -ne 0 ]; then
	            return "$@VAR_publish_result@"
	        fi
	        return 1
	    fi
	    rm -f -- "$@VAR_command_file@" || return 1
	    if ! {cd} "$@VAR_working_directory@"; then
	        local @VAR_publish_result@=0
	        functerm_publish_done "$@VAR_command_id@" 1 "0ns" "$PWD" "$@VAR_native_directory@" || @VAR_publish_result@=$?
	        functerm_restore_command_environment \
	            "$@VAR_had_previous_command_id@" "$@VAR_previous_command_id@" \
	            "$@VAR_had_previous_command_directory@" "$@VAR_previous_command_directory@"
	        if [ "$@VAR_publish_result@" -ne 0 ]; then
	            return "$@VAR_publish_result@"
	        fi
	        return 1
	    fi
	    if ! functerm_publish_start "$@VAR_command_id@" "$@VAR_native_directory@"; then
	        local @VAR_publish_result@=0
	        functerm_publish_done "$@VAR_command_id@" 1 "0ns" "$PWD" "$@VAR_native_directory@" || @VAR_publish_result@=$?
	        functerm_restore_command_environment \
	            "$@VAR_had_previous_command_id@" "$@VAR_previous_command_id@" \
	            "$@VAR_had_previous_command_directory@" "$@VAR_previous_command_directory@"
	        if [ "$@VAR_publish_result@" -ne 0 ]; then
	            return "$@VAR_publish_result@"
	        fi
	        return 1
	    fi
	    local @VAR_command_started_at@="$(functerm_command_time_millis)" || return 1
	    {{ eval "$@VAR_script@"; }} > "$@VAR_stdout_file@" 2> "$@VAR_stderr_file@"
	    local @VAR_exit_code@=$?
	{environment_restore}
	    local @VAR_command_finished_at@="$(functerm_command_time_millis)" || return 1
	    local @VAR_time_consumption@="$((@VAR_command_finished_at@ - @VAR_command_started_at@))ms"
	    cat "$@VAR_stdout_file@"
	    cat "$@VAR_stderr_file@" >&2
	    {mkdir} "$@VAR_state_dir@" || return 1
	    if ! functerm_publish_done "$@VAR_command_id@" "$@VAR_exit_code@" "$@VAR_time_consumption@" "$PWD" "$@VAR_native_directory@"; then
	        functerm_restore_command_environment \
	            "$@VAR_had_previous_command_id@" "$@VAR_previous_command_id@" \
	            "$@VAR_had_previous_command_directory@" "$@VAR_previous_command_directory@"
	        return 1
	    fi
	    functerm_restore_command_environment \
	        "$@VAR_had_previous_command_id@" "$@VAR_previous_command_id@" \
	        "$@VAR_had_previous_command_directory@" "$@VAR_previous_command_directory@"
	    return "$@VAR_exit_code@"
}}
functerm_restore_command_environment() {{
{emulate}    if {test_one}; then
        export {command_id_env}="$2"
    else
        unset {command_id_env}
    fi
    if {test_three}; then
        export {command_dir_env}="$4"
    else
        unset {command_dir_env}
    fi
}}
functerm_publish_done() {{
{emulate}    local @VAR_command_id@="$1"
	    local @VAR_exit_code@="$2"
	    local @VAR_time_consumption@="$3"
	    local @VAR_cwd@="$4"
	    local @VAR_native_directory@="$5"
	    local @VAR_helper@="${{{helper_env}-}}"
	    if [ -e "$@VAR_done_file@" ]; then
	        return 0
	    fi
	    if [ -z "$@VAR_helper@" ]; then
	        printf '%s is not set\n' "{helper_env}" >&2
	        return 1
	    fi
	    @VAR_helper@="$(functerm_posix_path "$@VAR_helper@")" || return 1
	    "$@VAR_helper@" internal-write-done \
	        --command-id "$@VAR_command_id@" \
	        --exit-code "$@VAR_exit_code@" \
	        --time-consumption "$@VAR_time_consumption@" \
	        --cwd "$@VAR_cwd@" \
	        --directory "$@VAR_native_directory@"
}}
{publish_start}
functerm_command_time_millis() {{ {emulate}    perl -MTime::HiRes=time -e 'printf "%.0f\n", time() * 1000'; }}
functerm_ensure_shims() {{
{emulate}    local @VAR_shim_dir@="${{FUNCTERM_SHIM_DIR-}}"
	    if [ -z "$@VAR_shim_dir@" ]; then
	        return 0
	    fi
	    local @VAR_helper@="${{{helper_env}-}}"
	    if [ -z "$@VAR_helper@" ]; then
	        printf '%s is not set\n' "{helper_env}" >&2
	        return 1
	    fi
	    @VAR_helper@="$(functerm_posix_path "$@VAR_helper@")" || return 1
	    "$@VAR_helper@" internal-ensure-shims --directory "$@VAR_shim_dir@"
}}"#,
        name = POSIX_COMMAND_FUNCTION,
        emulate = dialect.emulate(),
        mkdir = dialect.mkdir(),
        input_dir = COMMAND_INPUT_DIRECTORY,
        output_dir = COMMAND_OUTPUT_DIRECTORY,
        state_dir = COMMAND_STATE_DIRECTORY,
        stdout = STDOUT_FILE,
        stderr = STDERR_FILE,
        command_file = COMMAND_FILE,
        done = DONE_FILE,
        helper_env = HELPER_EXECUTABLE_ENV,
        command_id_env = COMMAND_ID_ENV,
        command_dir_env = COMMAND_DIRECTORY_ENV,
        previous_flags = dialect.previous_flags(),
        environment_snapshot = super::variables::posix_environment_snapshot(),
        environment_restore = super::variables::posix_environment_restore(),
        cd = dialect.cd(),
        test_one = dialect.test_arg("1"),
        test_three = dialect.test_arg("3"),
        publish_start = super::start::posix(dialect),
    )
}
