set +o history
unset HISTFILE
export HISTSIZE=0
export HISTFILESIZE=0
history -c

mcp_pty_command() {
    local command_id="$1"
    local payload="$2"
    local directory="$3"
    local working_directory="$4"
    mkdir -p "$directory" || return 1
    local stdout_file="$directory/stdout.txt"
    local stderr_file="$directory/stderr.txt"
    local done_file="$directory/done.json"
    local done_temp_file="$directory/done.json.tmp"
    : > "$stdout_file"
    : > "$stderr_file"
    local script
    if ! script="$(printf '%s' "$payload" | base64 --decode 2> "$stderr_file")"; then
        local cwd_json
        cwd_json="$(mcp_pty_json_string "$PWD")"
        printf '{"command_id":"%s","exit_code":1,"cwd":%s,"completed_at":"%s"}\n' \
            "$command_id" "$cwd_json" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$done_temp_file"
        mv "$done_temp_file" "$done_file"
        cat "$stderr_file" >&2
        return 1
    fi
    cd "$working_directory" || return 1
    { eval "$script"; } > >(tee "$stdout_file") 2> >(tee "$stderr_file" >&2)
    local exit_code=$?
    local cwd_json
    cwd_json="$(mcp_pty_json_string "$PWD")"
    printf '{"command_id":"%s","exit_code":%s,"cwd":%s,"completed_at":"%s"}\n' \
        "$command_id" "$exit_code" "$cwd_json" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$done_temp_file"
    mv "$done_temp_file" "$done_file"
    return "$exit_code"
}

mcp_pty_json_string() {
    local value="$1"
    value="${value//\\/\\\\}"
    value="${value//\"/\\\"}"
    value="${value//$'\n'/\\n}"
    value="${value//$'\r'/\\r}"
    printf '"%s"' "$value"
}
