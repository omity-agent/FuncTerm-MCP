def mcp_pty_command [command_id: string, payload: string, directory: path] {
    mkdir $directory
    let stdout_file = ($directory | path join 'stdout.txt')
    let stderr_file = ($directory | path join 'stderr.txt')
    let done_file = ($directory | path join 'done.json')
    '' | save --force --raw $stdout_file
    '' | save --force --raw $stderr_file
    let exit_code = try {
        let script = ($payload | decode base64 | decode)
        do --ignore-errors { nu --commands $script out> $stdout_file err> $stderr_file }
        let command_exit_code = if ($env.LAST_EXIT_CODE? | is-empty) { 0 } else { $env.LAST_EXIT_CODE }
        if ($stdout_file | path exists) {
            print --raw --no-newline (open --raw $stdout_file)
        }
        if ($stderr_file | path exists) {
            print --raw --no-newline --stderr (open --raw $stderr_file)
        }
        $command_exit_code
    } catch {|error|
        $error.msg | save --append --raw $stderr_file
        print --stderr $error.msg
        1
    }
    {
        command_id: $command_id,
        exit_code: $exit_code,
        cwd: $env.PWD,
        completed_at: (date now | date to-timezone UTC | format date '%+')
    } | to json --raw | save --force $done_file
}
