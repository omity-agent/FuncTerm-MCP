use super::posix_dialect::PosixDialect;
use super::posix_startup::{path_function, shim_path_function};
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV, COMMAND_PAYLOAD_FILE, DONE_FILE, HELPER_EXECUTABLE_ENV,
    POSIX_COMMAND_FUNCTION, STARTED_FILE, STDERR_FILE, STDOUT_FILE,
};
pub(in crate::shell) fn bash_wrapper() -> String {
    format!(
        "set +o history
unset HISTFILE
export HISTSIZE=0
export HISTFILESIZE=0
history -c
{path}
{shim_path}
{command}
",
        path = path_function(false),
        shim_path = shim_path_function(false),
        command = command_function(PosixDialect::Bash)
    )
}
pub(in crate::shell) fn zsh_wrapper() -> String {
    format!(
        "unset HISTFILE
HISTSIZE=0
SAVEHIST=0
setopt no_append_history
setopt no_share_history
setopt no_inc_append_history
fc -p /dev/null 0 0 2> /dev/null || true
unset HISTFILE
{path}
{shim_path}
{command}
",
        path = path_function(true),
        shim_path = shim_path_function(true),
        command = command_function(PosixDialect::Zsh)
    )
}
fn command_function(dialect: PosixDialect) -> String {
    format!(
        r#"{name}() {{
{emulate}    local command_id="$1"
    local native_directory="$2"
    local directory="$native_directory"
    local working_directory="$3"
    directory="$(functerm_posix_path "$directory")" || return 1
    working_directory="$(functerm_posix_path "$working_directory")" || return 1
    {mkdir} "$directory" || return 1
    local stdout_file="$directory/{stdout}"
    local stderr_file="$directory/{stderr}"
    local started_file="$directory/{started}"
    local payload_file="$directory/{payload}"
    local done_file="$directory/{done}"
    local previous_command_id="${{{command_id_env}-}}"
    local previous_command_directory="${{{command_dir_env}-}}"
{previous_flags}
    export {command_id_env}="$command_id"
    export {command_dir_env}="$directory"
    if ! functerm_ensure_shims; then
        local publish_result=0
        functerm_publish_done "$command_id" 1 "$PWD" "$native_directory" || publish_result=$?
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
    if ! script="$(functerm_decode_payload_file "$payload_file" "$stderr_file")"; then
        local publish_result=0
        functerm_publish_done "$command_id" 1 "$PWD" "$native_directory" || publish_result=$?
        cat "$stderr_file" >&2
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        if [ "$publish_result" -ne 0 ]; then
            return "$publish_result"
        fi
        return 1
    fi
    rm -f -- "$payload_file" || return 1
    if ! {cd} "$working_directory"; then
        local publish_result=0
        functerm_publish_done "$command_id" 1 "$PWD" "$native_directory" || publish_result=$?
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        if [ "$publish_result" -ne 0 ]; then
            return "$publish_result"
        fi
        return 1
    fi
    : {write_started} "$started_file"
    {{ eval "$script"; }} > "$stdout_file" 2> "$stderr_file"
    local exit_code=$?
    cat "$stdout_file"
    cat "$stderr_file" >&2
    {mkdir} "$directory" || return 1
    if ! functerm_publish_done "$command_id" "$exit_code" "$PWD" "$native_directory"; then
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
functerm_decode_payload_file() {{
{emulate}    local payload_file="$1"
    local stderr_file="$2"
    if base64 --decode < "$payload_file" 2> "$stderr_file"; then
        return 0
    fi
    base64 -D < "$payload_file" 2> "$stderr_file"
}}
functerm_publish_done() {{
{emulate}    local command_id="$1"
    local exit_code="$2"
    local cwd="$3"
    local native_directory="$4"
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
        --cwd "$cwd" \
        --directory "$native_directory"
}}
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
        stdout = STDOUT_FILE,
        stderr = STDERR_FILE,
        started = STARTED_FILE,
        payload = COMMAND_PAYLOAD_FILE,
        done = DONE_FILE,
        helper_env = HELPER_EXECUTABLE_ENV,
        command_id_env = COMMAND_ID_ENV,
        command_dir_env = COMMAND_DIRECTORY_ENV,
        previous_flags = dialect.previous_flags(),
        write_started = dialect.write_done_temp(),
        cd = dialect.cd(),
        test_one = dialect.test_arg("1"),
        test_three = dialect.test_arg("3"),
    )
}
