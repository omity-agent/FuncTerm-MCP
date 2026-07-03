use super::posix_dialect::PosixDialect;
use super::posix_startup::{path_function, shim_path_function};
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV, COMMAND_PAYLOAD_FILE, DONE_FILE, DONE_TEMP_FILE,
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
    local directory="$2"
    local working_directory="$3"
    directory="$(functerm_posix_path "$directory")" || return 1
    working_directory="$(functerm_posix_path "$working_directory")" || return 1
    {mkdir} "$directory" || return 1
    local stdout_file="$directory/{stdout}"
    local stderr_file="$directory/{stderr}"
    local started_file="$directory/{started}"
    local payload_file="$directory/{payload}"
    local done_file="$directory/{done}"
    local done_temp_file="$directory/{done_temp}"
    local previous_command_id="${{{command_id_env}-}}"
    local previous_command_directory="${{{command_dir_env}-}}"
{previous_flags}
    export {command_id_env}="$command_id"
    export {command_dir_env}="$directory"
    functerm_prepend_shim_path || return 1
    local script
    if ! script="$(functerm_decode_payload_file "$payload_file" "$stderr_file")"; then
        local cwd_json
        cwd_json="$(functerm_json_string "$PWD")"
        functerm_publish_done "$command_id" 1 "$cwd_json" "$done_file" "$done_temp_file"
        cat "$stderr_file" >&2
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        return 1
    fi
    rm -f -- "$payload_file" || return 1
    if ! {cd} "$working_directory"; then
        local cwd_json
        cwd_json="$(functerm_json_string "$PWD")"
        functerm_publish_done "$command_id" 1 "$cwd_json" "$done_file" "$done_temp_file"
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        return 1
    fi
    : {write_started} "$started_file"
    {{ eval "$script"; }} > "$stdout_file" 2> "$stderr_file"
    local exit_code=$?
    cat "$stdout_file"
    cat "$stderr_file" >&2
    local cwd_json
    cwd_json="$(functerm_json_string "$PWD")"
    {mkdir} "$directory" || return 1
    functerm_publish_done "$command_id" "$exit_code" "$cwd_json" "$done_file" "$done_temp_file"
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
functerm_json_string() {{
{emulate}    local value="$1"
    value="${{value//\\/\\\\}}"
    value="${{value//\"/\\\"}}"
    value="${{value//$'\b'/\\b}}"
    value="${{value//$'\t'/\\t}}"
    value="${{value//$'\n'/\\n}}"
    value="${{value//$'\f'/\\f}}"
    value="${{value//$'\r'/\\r}}"
    printf '"%s"' "$value"
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
    local cwd_json="$3"
    local done_file="$4"
    local done_temp_file="$5"
    if [ -e "$done_file" ]; then
        return 0
    fi
    printf '{{"command_id":"%s","exit_code":%s,"cwd":%s,"completed_at":"%s"}}\n' \
        "$command_id" "$exit_code" "$cwd_json" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" {write_done_temp} "$done_temp_file"
    {move_done}
}}"#,
        name = POSIX_COMMAND_FUNCTION,
        emulate = dialect.emulate(),
        mkdir = dialect.mkdir(),
        stdout = STDOUT_FILE,
        stderr = STDERR_FILE,
        started = STARTED_FILE,
        payload = COMMAND_PAYLOAD_FILE,
        done = DONE_FILE,
        done_temp = DONE_TEMP_FILE,
        command_id_env = COMMAND_ID_ENV,
        command_dir_env = COMMAND_DIRECTORY_ENV,
        previous_flags = dialect.previous_flags(),
        write_done_temp = dialect.write_done_temp(),
        write_started = dialect.write_done_temp(),
        move_done = dialect.move_done(),
        cd = dialect.cd(),
        test_one = dialect.test_arg("1"),
        test_three = dialect.test_arg("3"),
    )
}
