use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV, COMMAND_PAYLOAD_FILE, DONE_FILE, DONE_TEMP_FILE,
    HELPER_EXECUTABLE_ENV, POSIX_COMMAND_FUNCTION, STARTED_FILE, STDERR_FILE, STDOUT_FILE,
};
pub(in crate::shell) fn wrapper() -> String {
    substitute(
        TEMPLATE,
        &[
            ("@COMMAND_DIR_ENV@", COMMAND_DIRECTORY_ENV),
            ("@COMMAND_ID_ENV@", COMMAND_ID_ENV),
            ("@DONE@", DONE_FILE),
            ("@DONE_TEMP@", DONE_TEMP_FILE),
            ("@FUNCTION@", POSIX_COMMAND_FUNCTION),
            ("@HELPER_ENV@", HELPER_EXECUTABLE_ENV),
            ("@PAYLOAD@", COMMAND_PAYLOAD_FILE),
            ("@STDERR@", STDERR_FILE),
            ("@STARTED@", STARTED_FILE),
            ("@STDOUT@", STDOUT_FILE),
        ],
    )
}
fn substitute(template: &str, pairs: &[(&str, &str)]) -> String {
    let mut text = template.to_owned();
    for &(placeholder, value) in pairs {
        text = text.replace(placeholder, value);
    }
    text
}
const TEMPLATE: &str = r#"def @FUNCTION@ [command_id: string, directory: path, working_directory: path] {
    mkdir $directory
    let stdout_file = ($directory | path join '@STDOUT@')
    let stderr_file = ($directory | path join '@STDERR@')
    let started_file = ($directory | path join '@STARTED@')
    let payload_file = ($directory | path join '@PAYLOAD@')
    let done_file = ($directory | path join '@DONE@')
    let done_temp_file = ($directory | path join '@DONE_TEMP@')
    let state_file = ($directory | path join 'state.json')
    let script_file = ($directory | path join 'command.nu')
    let previous_command_id = $env.@COMMAND_ID_ENV@?
    let previous_command_directory = $env.@COMMAND_DIR_ENV@?
    $env.@COMMAND_ID_ENV@ = $command_id
    $env.@COMMAND_DIR_ENV@ = ($directory | path expand)
    let state = try {
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
    mkdir $directory
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
