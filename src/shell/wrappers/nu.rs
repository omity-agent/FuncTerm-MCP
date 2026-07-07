use super::template;
use crate::contract::POSIX_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    template::render_payload_function(TEMPLATE, POSIX_COMMAND_FUNCTION)
}
const TEMPLATE : & str = "def @FUNCTION@ [command_id: string, directory: path, working_directory: path] {
    let input_dir = ($directory | path join '@INPUT_DIR@')
    let output_dir = ($directory | path join '@OUTPUT_DIR@')
    let state_dir = ($directory | path join '@STATE_DIR@')
    mkdir $input_dir $output_dir $state_dir
    let stdout_file = ($output_dir | path join '@STDOUT@')
    let stderr_file = ($output_dir | path join '@STDERR@')
    let started_file = ($state_dir | path join '@STARTED@')
    let payload_file = ($input_dir | path join '@PAYLOAD@')
    let done_file = ($state_dir | path join '@DONE@')
    let cwd_file = ($state_dir | path join 'nushell-cwd.txt')
    let config_file = ($input_dir | path join 'functerm-config.nu')
    let env_config_file = ($input_dir | path join 'functerm-env.nu')
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
            $env.PATH = ($env.PATH | where {|entry| $entry != $shim_dir } | prepend $shim_dir)
        }
        let payload = (open --raw $payload_file)
        $payload | decode base64 | decode | save --force --raw $script_file
        rm --force $payload_file
        let previous_pwd = $env.PWD
        cd $working_directory
        $env.PWD | save --force --raw $cwd_file
        $'' | save --force --raw $env_config_file
        [
            '$env.config.show_banner = false'
            '$env.config.use_ansi_coloring = false'
            '$env.config.shell_integration = { osc2: false, osc7: false, osc8: false, osc9_9: false, osc133: false, osc633: false, reset_application_mode: false }'
            '$env.config.history.file_format = \"plaintext\"'
            '$env.config.history.max_size = 0'
            '$env.config.history.sync_on_enter = false'
            'if not ($env.FUNCTERM_SHIM_DIR? | is-empty) { $env.PATH = ($env.PATH | where {|entry| $entry != $env.FUNCTERM_SHIM_DIR } | prepend $env.FUNCTERM_SHIM_DIR) }'
            $'$env.config.hooks.env_change.PWD = [{|before, after| $after | save --force --raw ($cwd_file | to nuon) }]'
            $'$env.config.hooks.pre_prompt = [{|| $env.PWD | save --force --raw ($cwd_file | to nuon); exit (if ($env.LAST_EXIT_CODE? | is-empty) { 0 } else { $env.LAST_EXIT_CODE }) }]'
        ] | str join (char newline) | save --force --raw $config_file
        let command = $'source ($script_file | to nuon)'
        let nushell = $env.FUNCTERM_REAL_NUSHELL?
        if ($nushell | is-empty) {
            error make {msg: 'FUNCTERM_REAL_NUSHELL is not set'}
        }
        '' | save --force --raw $started_file
        let result = (^$nushell --no-history --config $config_file --env-config $env_config_file --interactive --execute $command | complete)
        $result.stdout | save --force --raw $stdout_file
        $result.stderr | save --force --raw $stderr_file
        cd $previous_pwd
        if ($stdout_file | path exists) {
            print --raw --no-newline (open --raw $stdout_file)
        }
        if ($stderr_file | path exists) {
            print --raw --no-newline --stderr (open --raw $stderr_file)
        }
        rm --force $script_file
        rm --force $config_file
        rm --force $env_config_file
        let command_cwd = if ($cwd_file | path exists) {
            open --raw $cwd_file
        } else {
            $working_directory | path expand
        }
        rm --force $cwd_file
        { cwd: $command_cwd, exit_code: $result.exit_code }
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
" ;
