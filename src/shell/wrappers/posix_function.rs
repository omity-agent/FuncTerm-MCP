use super::posix_dialect::PosixDialect;
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, COMMAND_INPUT_DIRECTORY,
    COMMAND_OUTPUT_DIRECTORY, COMMAND_STATE_DIRECTORY, DONE_FILE, HELPER_EXECUTABLE_ENV,
    POSIX_COMMAND_FUNCTION, STDERR_FILE, STDOUT_FILE,
};
pub(super) fn command_function(dialect: PosixDialect) -> String {
    format!(
        r#"{name}() {{
	    local command_id="$1"
    local native_directory="$2"
    local directory="$native_directory"
    local working_directory="$3"
    directory="$(functerm_posix_path "$directory")" || return 1
    working_directory="$(functerm_posix_path "$working_directory")" || return 1
    local input_dir="$directory/{input_dir}"
    local output_dir="$directory/{output_dir}"
    local state_dir="$directory/{state_dir}"
    {mkdir} "$input_dir" "$output_dir" "$state_dir" || return 1
    local stdout_file="$output_dir/{stdout}"
    local stderr_file="$output_dir/{stderr}"
    local command_file="$input_dir/{command_file}"
    local done_file="$state_dir/{done}"
    local previous_command_id="${{{command_id_env}-}}"
    local previous_command_directory="${{{command_dir_env}-}}"
{previous_flags}
    export {command_id_env}="$command_id"
    export {command_dir_env}="$directory"
    if ! functerm_ensure_shims; then
        local publish_result=0
        functerm_publish_done "$command_id" 1 "0ns" "$PWD" "$native_directory" || publish_result=$?
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        if [ "$publish_result" -ne 0 ]; then
            return "$publish_result"
        fi
        return 1
    fi
    functerm_prepend_shim_path || return 1
    local script
    if ! script="$(cat "$command_file" 2> "$stderr_file")"; then
        local publish_result=0
        functerm_publish_done "$command_id" 1 "0ns" "$PWD" "$native_directory" || publish_result=$?
        cat "$stderr_file" >&2
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        if [ "$publish_result" -ne 0 ]; then
            return "$publish_result"
        fi
        return 1
    fi
    rm -f -- "$command_file" || return 1
    if ! {cd} "$working_directory"; then
        local publish_result=0
        functerm_publish_done "$command_id" 1 "0ns" "$PWD" "$native_directory" || publish_result=$?
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        if [ "$publish_result" -ne 0 ]; then
            return "$publish_result"
        fi
        return 1
    fi
    if ! functerm_publish_start "$command_id" "$native_directory"; then
        local publish_result=0
        functerm_publish_done "$command_id" 1 "0ns" "$PWD" "$native_directory" || publish_result=$?
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        if [ "$publish_result" -ne 0 ]; then
            return "$publish_result"
        fi
        return 1
    fi
    local command_started_at="$(functerm_command_time_millis)" || return 1
    {{ eval "$script"; }} > "$stdout_file" 2> "$stderr_file"
    local exit_code=$?
    local command_finished_at="$(functerm_command_time_millis)" || return 1
    local time_consumption="$((command_finished_at - command_started_at))ms"
    cat "$stdout_file"
    cat "$stderr_file" >&2
    {mkdir} "$state_dir" || return 1
    if ! functerm_publish_done "$command_id" "$exit_code" "$time_consumption" "$PWD" "$native_directory"; then
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        return 1
    fi
    functerm_restore_command_environment \
        "$had_previous_command_id" "$previous_command_id" \
        "$had_previous_command_directory" "$previous_command_directory"
    return "$exit_code"
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
{emulate}    local command_id="$1"
    local exit_code="$2"
    local time_consumption="$3"
    local cwd="$4"
    local native_directory="$5"
    local helper="${{{helper_env}-}}"
    if [ -e "$done_file" ]; then
        return 0
    fi
    if [ -z "$helper" ]; then
        printf '%s is not set\n' "{helper_env}" >&2
        return 1
    fi
    helper="$(functerm_posix_path "$helper")" || return 1
    "$helper" internal-write-done \
        --command-id "$command_id" \
        --exit-code "$exit_code" \
        --time-consumption "$time_consumption" \
        --cwd "$cwd" \
        --directory "$native_directory"
}}
{publish_start}
functerm_command_time_millis() {{ {emulate}    perl -MTime::HiRes=time -e 'printf "%.0f\n", time() * 1000'; }}
functerm_ensure_shims() {{
{emulate}    local shim_dir="${{FUNCTERM_SHIM_DIR-}}"
    if [ -z "$shim_dir" ]; then
        return 0
    fi
    local helper="${{{helper_env}-}}"
    if [ -z "$helper" ]; then
        printf '%s is not set\n' "{helper_env}" >&2
        return 1
    fi
    helper="$(functerm_posix_path "$helper")" || return 1
    "$helper" internal-ensure-shims --directory "$shim_dir"
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
        cd = dialect.cd(),
        test_one = dialect.test_arg("1"),
        test_three = dialect.test_arg("3"),
        publish_start = super::start::posix(dialect),
    )
}
