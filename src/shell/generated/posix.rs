use super::posix_dialect::PosixDialect;
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV, COMMAND_PAYLOAD_FILE, DONE_FILE, DONE_TEMP_FILE,
    POSIX_COMMAND_FUNCTION, STDERR_FILE, STDOUT_FILE,
};
use crate::shell::shims::SHIM_DIR_ENV;
pub(in crate::shell) fn bash_wrapper() -> String {
    format!(
        "set +o history
unset HISTFILE
export HISTSIZE=0
export HISTFILESIZE=0
history -c

{shim_path}

{command}
",
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

{shim_path}

{command}
",
        shim_path = shim_path_function(true),
        command = command_function(PosixDialect::Zsh)
    )
}
fn shim_path_function(zsh: bool) -> String {
    let local_options = if zsh { "\n    emulate -L zsh" } else { "" };
    format!(
        r#"functerm_prepend_shim_path() {{{local_options}
    if [ -z "${{{SHIM_DIR_ENV}-}}" ]; then
        return 0
    fi
    local shim_dir="${{{SHIM_DIR_ENV}}}"
    if command -v cygpath > /dev/null 2>&1; then
        shim_dir="$(cygpath -u "$shim_dir" 2> /dev/null || printf '%s' "$shim_dir")"
    fi
    export PATH="$shim_dir:$PATH"
}}
functerm_prepend_shim_path"#
    )
}
fn command_function(dialect: PosixDialect) -> String {
    format!(
        r#"{name}() {{
{emulate}    local command_id="$1"
    local directory="$2"
    local working_directory="$3"
    {mkdir} "$directory" || return 1
    local stdout_file="$directory/{stdout}"
    local stderr_file="$directory/{stderr}"
    local payload_file="$directory/{payload}"
    local done_file="$directory/{done}"
    local done_temp_file="$directory/{done_temp}"
    local previous_command_id="${{{command_id_env}-}}"
    local previous_command_directory="${{{command_dir_env}-}}"
{previous_flags}
    export {command_id_env}="$command_id"
    export {command_dir_env}="$directory"
    {truncate} "$stdout_file"
    {truncate} "$stderr_file"
    local script
    if ! script="$(functerm_decode_payload_file "$payload_file" "$stderr_file")"; then
        local cwd_json
        cwd_json="$(functerm_json_string "$PWD")"
        printf '{{"command_id":"%s","exit_code":1,"cwd":%s,"completed_at":"%s"}}\n' \
            "$command_id" "$cwd_json" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" {write_done_temp} "$done_temp_file"
        {move_done}
        cat "$stderr_file" >&2
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        return 1
    fi
    if ! {cd} "$working_directory"; then
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        return 1
    fi
    {{ eval "$script"; }} > >(tee "$stdout_file") 2> >(tee "$stderr_file" >&2)
    local exit_code=$?
    local cwd_json
    cwd_json="$(functerm_json_string "$PWD")"
    printf '{{"command_id":"%s","exit_code":%s,"cwd":%s,"completed_at":"%s"}}\n' \
        "$command_id" "$exit_code" "$cwd_json" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" {write_done_temp} "$done_temp_file"
    {move_done}
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
    value="${{value//$'\n'/\\n}}"
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
}}"#,
        name = POSIX_COMMAND_FUNCTION,
        emulate = dialect.emulate(),
        mkdir = dialect.mkdir(),
        stdout = STDOUT_FILE,
        stderr = STDERR_FILE,
        payload = COMMAND_PAYLOAD_FILE,
        done = DONE_FILE,
        done_temp = DONE_TEMP_FILE,
        command_id_env = COMMAND_ID_ENV,
        command_dir_env = COMMAND_DIRECTORY_ENV,
        previous_flags = dialect.previous_flags(),
        truncate = dialect.truncate(),
        write_done_temp = dialect.write_done_temp(),
        move_done = dialect.move_done(),
        cd = dialect.cd(),
        test_one = dialect.test_arg("1"),
        test_three = dialect.test_arg("3"),
    )
}
