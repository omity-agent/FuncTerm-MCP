def mcp_pty_command [command_id: string, payload: string, directory: path, working_directory: path] {
    mkdir $directory
    let stdout_file = ($directory | path join 'stdout.txt')
    let stderr_file = ($directory | path join 'stderr.txt')
    let done_file = ($directory | path join 'done.json')
    let state_file = ($directory | path join 'state.json')
    let script_file = ($directory | path join 'command.nu')
    '' | save --force --raw $stdout_file
    '' | save --force --raw $stderr_file
    let state = try {
        let script = ($payload | decode base64 | decode)
        [
            $"cd ($working_directory | to nuon)"
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
    } | to json --raw | save --force $done_file
}
