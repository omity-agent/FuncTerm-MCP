mcp_pty_command() {
    local command_id="$1"
    local payload="$2"
    local directory="$3"
    mkdir -p "$directory" || return 1
    local stdout_file="$directory/stdout.txt"
    local stderr_file="$directory/stderr.txt"
    local done_file="$directory/done.json"
    : > "$stdout_file"
    : > "$stderr_file"
    local script
    if ! script="$(printf '%s' "$payload" | base64 --decode 2> "$stderr_file")"; then
        printf '{"command_id":"%s","exit_code":1,"completed_at":"%s"}\n' \
            "$command_id" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$done_file"
        cat "$stderr_file" >&2
        return 1
    fi
    { eval "$script"; } > >(tee "$stdout_file") 2> >(tee "$stderr_file" >&2)
    local exit_code=$?
    printf '{"command_id":"%s","exit_code":%s,"completed_at":"%s"}\n' \
        "$command_id" "$exit_code" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" > "$done_file"
    return "$exit_code"
}
