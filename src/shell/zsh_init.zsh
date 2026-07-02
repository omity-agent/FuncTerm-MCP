unset HISTFILE
HISTSIZE=0
SAVEHIST=0
setopt no_append_history
setopt no_share_history
setopt no_inc_append_history
fc -p /dev/null 0 0 2> /dev/null || true
unset HISTFILE

functerm_prepend_shim_path() {
    emulate -L zsh
    if [[ -z "${FUNCTERM_SHIM_DIR-}" ]]; then
        return 0
    fi
    local shim_dir="$FUNCTERM_SHIM_DIR"
    if command -v cygpath > /dev/null 2>&1; then
        shim_dir="$(cygpath -u "$shim_dir" 2> /dev/null || printf '%s' "$shim_dir")"
    fi
    export PATH="$shim_dir:$PATH"
}
functerm_prepend_shim_path

functerm_run_command() {
    emulate -L zsh
    local command_id="$1"
    local directory="$2"
    local working_directory="$3"
    mkdir -p -- "$directory" || return 1
    local stdout_file="$directory/stdout.txt"
    local stderr_file="$directory/stderr.txt"
    local payload_file="$directory/command.b64"
    local done_file="$directory/done.json"
    local done_temp_file="$directory/done.json.tmp"
    local previous_command_id="${FUNCTERM_COMMAND_ID-}"
    local previous_command_directory="${FUNCTERM_COMMAND_DIRECTORY-}"
    local had_previous_command_id=${+FUNCTERM_COMMAND_ID}
    local had_previous_command_directory=${+FUNCTERM_COMMAND_DIRECTORY}
    export FUNCTERM_COMMAND_ID="$command_id"
    export FUNCTERM_COMMAND_DIRECTORY="$directory"
    : >| "$stdout_file"
    : >| "$stderr_file"
    local script
    if ! script="$(functerm_decode_payload_file "$payload_file" "$stderr_file")"; then
        local cwd_json
        cwd_json="$(functerm_json_string "$PWD")"
        printf '{"command_id":"%s","exit_code":1,"cwd":%s,"completed_at":"%s"}\n' \
            "$command_id" "$cwd_json" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >| "$done_temp_file"
        mv -f -- "$done_temp_file" "$done_file"
        cat "$stderr_file" >&2
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        return 1
    fi
    if ! builtin cd -- "$working_directory"; then
        functerm_restore_command_environment \
            "$had_previous_command_id" "$previous_command_id" \
            "$had_previous_command_directory" "$previous_command_directory"
        return 1
    fi
    { eval "$script"; } > >(tee "$stdout_file") 2> >(tee "$stderr_file" >&2)
    local exit_code=$?
    local cwd_json
    cwd_json="$(functerm_json_string "$PWD")"
    printf '{"command_id":"%s","exit_code":%s,"cwd":%s,"completed_at":"%s"}\n' \
        "$command_id" "$exit_code" "$cwd_json" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >| "$done_temp_file"
    mv -f -- "$done_temp_file" "$done_file"
    functerm_restore_command_environment \
        "$had_previous_command_id" "$previous_command_id" \
        "$had_previous_command_directory" "$previous_command_directory"
    return "$exit_code"
}

functerm_restore_command_environment() {
    emulate -L zsh
    if [[ "$1" == 1 ]]; then
        export FUNCTERM_COMMAND_ID="$2"
    else
        unset FUNCTERM_COMMAND_ID
    fi
    if [[ "$3" == 1 ]]; then
        export FUNCTERM_COMMAND_DIRECTORY="$4"
    else
        unset FUNCTERM_COMMAND_DIRECTORY
    fi
}

functerm_json_string() {
    emulate -L zsh
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\r'/\\r}"
    printf '"%s"' "$value"
}

functerm_decode_payload_file() {
    emulate -L zsh
    local payload_file="$1"
    local stderr_file="$2"
    if base64 --decode < "$payload_file" 2> "$stderr_file"; then
        return 0
    fi
    base64 -D < "$payload_file" 2> "$stderr_file"
}
