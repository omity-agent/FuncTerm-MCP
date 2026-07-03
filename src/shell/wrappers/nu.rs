use super::template;
use crate::contract::POSIX_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    template::render_payload_function(TEMPLATE, POSIX_COMMAND_FUNCTION)
}
const TEMPLATE: &str = r#"def @FUNCTION@ [command_id: string, directory: path, working_directory: path] {
    let input_dir = ($directory | path join '@INPUT_DIR@')
    let output_dir = ($directory | path join '@OUTPUT_DIR@')
    let state_dir = ($directory | path join '@STATE_DIR@')
    mkdir $input_dir $output_dir $state_dir
    let stdout_file = ($output_dir | path join '@STDOUT@')
    let stderr_file = ($output_dir | path join '@STDERR@')
    let started_file = ($state_dir | path join '@STARTED@')
    let payload_file = ($input_dir | path join '@PAYLOAD@')
    let done_file = ($state_dir | path join '@DONE@')
    let state_file = ($state_dir | path join 'nushell-state.json')
    let script_file = ($input_dir | path join 'command.nu')
    let previous_command_id = $env.@COMMAND_ID_ENV@?
    let previous_command_directory = $env.@COMMAND_DIR_ENV@?
    $env.@COMMAND_ID_ENV@ = $command_id
    $env.@COMMAND_DIR_ENV@ = ($directory | path expand)
    let state = try {
        let shim_dir = $env.FUNCTERM_SHIM_DIR?
        if not ($shim_dir | is-empty) {
            let helper = $env.@HELPER_ENV@?
            if ($helper | is-empty) {
                error make {msg: '@HELPER_ENV@ is not set'}
            }
            ^$helper internal-ensure-shims --directory $shim_dir
            if $env.LAST_EXIT_CODE != 0 {
                error make {msg: 'failed to ensure FuncTerm shell shims'}
            }
        }
        let payload = (open --raw $payload_file)
        let script = ($payload | decode base64 | decode)
        rm --force $payload_file
        [
            $"cd ($working_directory | to nuon)"
            $"$env.@COMMAND_ID_ENV@ = ($command_id | to nuon)"
            $"$env.@COMMAND_DIR_ENV@ = ($directory | to nuon)"
            $script
            "let mcp_exit_code = if ($env.LAST_EXIT_CODE? | is-empty) { 0 } else { $env.LAST_EXIT_CODE }"
            $"{ cwd: $env.PWD, exit_code: $mcp_exit_code } | to json --raw | save --force ($state_file | to nuon)"
            "exit $mcp_exit_code"
        ] | str join (char newline) | save --force --raw $script_file
        '' | save --force --raw $started_file
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
        rm --force $script_file
        rm --force $state_file
        $command_state
    } catch {|error|
        $error.msg | save --append --raw $stderr_file
        print --stderr $error.msg
        { cwd: ($working_directory | path expand), exit_code: 1 }
    }
    mkdir $state_dir
    if not ($done_file | path exists) {
        let helper = $env.@HELPER_ENV@?
        if ($helper | is-empty) {
            print --stderr '@HELPER_ENV@ is not set'
            return 1
        }
        ^$helper internal-write-done --command-id $command_id --exit-code $state.exit_code --cwd $state.cwd --directory $directory
    }
    if ($previous_command_id | is-empty) {
        hide-env @COMMAND_ID_ENV@
    } else {
        $env.@COMMAND_ID_ENV@ = $previous_command_id
    }
    if ($previous_command_directory | is-empty) {
        hide-env @COMMAND_DIR_ENV@
    } else {
        $env.@COMMAND_DIR_ENV@ = $previous_command_directory
    }
}
"#;
