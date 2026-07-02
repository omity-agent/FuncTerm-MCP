def functerm_run_command [command_id: string, directory: path, working_directory: path] {
    mkdir $directory
    let stdout_file = ($directory | path join 'stdout.txt')
    let stderr_file = ($directory | path join 'stderr.txt')
    let payload_file = ($directory | path join 'command.b64')
    let done_file = ($directory | path join 'done.json')
    let done_temp_file = ($directory | path join 'done.json.tmp')
    let state_file = ($directory | path join 'state.json')
    let script_file = ($directory | path join 'command.nu')
    let previous_command_id = $env.FUNCTERM_COMMAND_ID?
    let previous_command_directory = $env.FUNCTERM_COMMAND_DIRECTORY?
    $env.FUNCTERM_COMMAND_ID = $command_id
    $env.FUNCTERM_COMMAND_DIRECTORY = ($directory | path expand)
    '' | save --force --raw $stdout_file
    '' | save --force --raw $stderr_file
    let state = try {
        let payload = (open --raw $payload_file)
        let script = ($payload | decode base64 | decode)
        [
            $"cd ($working_directory | to nuon)"
            $"$env.FUNCTERM_COMMAND_ID = ($command_id | to nuon)"
            $"$env.FUNCTERM_COMMAND_DIRECTORY = ($directory | to nuon)"
            $script
            "let mcp_exit_code = if ($env.LAST_EXIT_CODE? | is-empty) { 0 } else { $env.LAST_EXIT_CODE }"
            $"{ cwd: $env.PWD, exit_code: $mcp_exit_code } | to json --raw | save --force ($state_file | to nuon)"
            "exit $mcp_exit_code"
        ] | str join (char newline) | save --force --raw $script_file
        do --ignore-errors { nu $script_file out> $stdout_file err> $stderr_file }
        let process_exit_code = if ($env.LAST_EXIT_CODE? | is-empty) { 1 } else { $env.LAST_EXIT_CODE }
        let command_state = if ($state_file | path exists) {
            open $state_file
        } else {
            { cwd: ($working_directory | path expand), exit_code: $process_exit_code }
        }
        if ($stdout_file | path exists) {
            print --raw --no-newline (open --raw $stdout_file)
        }
        if ($stderr_file | path exists) {
            print --raw --no-newline --stderr (open --raw $stderr_file)
        }
        $command_state
    } catch {|error|
        $error.msg | save --append --raw $stderr_file
        print --stderr $error.msg
        { cwd: ($working_directory | path expand), exit_code: 1 }
    }
    {
        command_id: $command_id,
        exit_code: $state.exit_code,
        cwd: $state.cwd,
        completed_at: (date now | date to-timezone UTC | format date '%+')
    } | to json --raw | save --force $done_temp_file
    mv --force $done_temp_file $done_file
    if ($previous_command_id | is-empty) {
        hide-env FUNCTERM_COMMAND_ID
    } else {
        $env.FUNCTERM_COMMAND_ID = $previous_command_id
    }
    if ($previous_command_directory | is-empty) {
        hide-env FUNCTERM_COMMAND_DIRECTORY
    } else {
        $env.FUNCTERM_COMMAND_DIRECTORY = $previous_command_directory
    }
}
