use super::template;
use crate::contract::POSIX_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    let stateful = TEMPLATE.replace("@NUSHELL_STATE_FUNCTIONS@", NUSHELL_STATE_FUNCTIONS);
    let runner = template::render_command_function(&stateful, POSIX_COMMAND_FUNCTION);
    format!("{runner}\n{}", super::template::nushell_dispatcher())
}
const TEMPLATE : & str = "def --env @FUNCTION@ [command_id: string, directory: path, working_directory: path] {
    let input_dir = ($directory | path join '@INPUT_DIR@')
    let output_dir = ($directory | path join '@OUTPUT_DIR@')
    let state_dir = ($directory | path join '@STATE_DIR@')
    mkdir $input_dir $output_dir $state_dir
    let stdout_file = ($output_dir | path join '@STDOUT@')
    let stderr_file = ($output_dir | path join '@STDERR@')
    let command_file = ($input_dir | path join '@COMMAND@')
    let done_file = ($state_dir | path join '@DONE@')
    let session_state_dir = ($env.FUNCTERM_SESSION_ROOT | path join 'state')
    let env_state_file = ($session_state_dir | path join 'nushell-env.nuon')
    let config_state_file = ($session_state_dir | path join 'nushell-config.nuon')
    let declaration_state_file = ($session_state_dir | path join 'nushell-declarations.nu')
    mkdir $session_state_dir
    if not ($declaration_state_file | path exists) {
        $'' | save --force --raw $declaration_state_file
    }
    let previous_env_names = if ($env_state_file | path exists) {
        open --raw $env_state_file | from nuon | columns
    } else {
        []
    }
    let cwd_file = ($state_dir | path join 'nushell-cwd.txt')
    let config_file = ($input_dir | path join 'functerm-config.nu')
    let env_config_file = ($input_dir | path join 'functerm-env.nu')
    let script_file = ($input_dir | path join 'command.nu')
    let previous_command_id = $env.@COMMAND_ID_ENV@?
    let previous_command_directory = $env.@COMMAND_DIR_ENV@?
    $env.@COMMAND_ID_ENV@ = $command_id
    $env.@COMMAND_DIR_ENV@ = ($directory | path expand)
    let state = try {
        ensure_nushell_shims
        open --raw $command_file | save --force --raw $script_file
        rm --force $command_file
        write_nushell_config $config_file $cwd_file $env_state_file $config_state_file $declaration_state_file
        $'' | save --force --raw $env_config_file
        let command = $'source (char single_quote)($declaration_state_file)(char single_quote); source (char single_quote)($script_file)(char single_quote)'
        let nushell = $env.FUNCTERM_REAL_NUSHELL?
        if ($nushell | is-empty) {
            error make {msg: 'FUNCTERM_REAL_NUSHELL is not set'}
        }
        let helper = $env.@HELPER_ENV@?
        if ($helper | is-empty) {
            error make {msg: '@HELPER_ENV@ is not set'}
        }
        ^$helper internal-write-start --command-id $command_id --directory $directory
        if $env.LAST_EXIT_CODE != 0 {
            error make {msg: 'failed to publish command start'}
        }
        let command_started_at = date now
        let result = (^$nushell --no-history --config $config_file --env-config $env_config_file --commands $command | complete)
        let time_consumption = ((date now) - $command_started_at) | into string
        $result.stdout | save --force --raw $stdout_file
        $result.stderr | save --force --raw $stderr_file
        if ($stdout_file | path exists) {
            print --raw --no-newline (open --raw $stdout_file)
        }
        if ($stderr_file | path exists) {
            print --raw --no-newline --stderr (open --raw $stderr_file)
        }
        rm --force $script_file $config_file $env_config_file
        let command_cwd = if ($cwd_file | path exists) {
            open --raw $cwd_file
        } else {
            $working_directory | path expand
        }
        rm --force $cwd_file
        restore_nushell_environment $env_state_file $config_state_file $previous_env_names
        { cwd: $command_cwd, exit_code: $result.exit_code, time_consumption: $time_consumption }
    } catch {|error|
        $error.msg | save --append --raw $stderr_file
        print --stderr $error.msg
        { cwd: ($working_directory | path expand), exit_code: 1, time_consumption: '0ns' }
    }
    mkdir $state_dir
    if not ($done_file | path exists) {
        let helper = $env.@HELPER_ENV@?
        if ($helper | is-empty) {
            print --stderr '@HELPER_ENV@ is not set'
            return 1
        }
        ^$helper internal-write-done --command-id $command_id --exit-code $state.exit_code --time-consumption $state.time_consumption --cwd $state.cwd --directory $directory
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
def --env ensure_nushell_shims [] {
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
}
def write_nushell_config [
    config_file: path,
    cwd_file: path,
    env_state_file: path,
    config_state_file: path,
    declaration_state_file: path,
] {
    [
        '$env.config.show_banner = false'
        '$env.config.use_ansi_coloring = false'
        '$env.config.shell_integration = { osc2: false, osc7: false, osc8: false, osc9_9: false, osc133: false, osc633: false, reset_application_mode: false }'
        '$env.config.history.file_format = \"plaintext\"'
        '$env.config.history.max_size = 0'
        '$env.config.history.sync_on_enter = false'
        $'let functerm_env_state_file = ($env_state_file | to nuon)'
        'if ($functerm_env_state_file | path exists) { load-env (open --raw $functerm_env_state_file | from nuon) }'
        $'let functerm_config_state_file = ($config_state_file | to nuon)'
        'if ($functerm_config_state_file | path exists) { $env.config = ($env.config | merge (open --raw $functerm_config_state_file | from nuon)) }'
        'if not ($env.FUNCTERM_SHIM_DIR? | is-empty) { $env.PATH = ($env.PATH | where {|entry| $entry != $env.FUNCTERM_SHIM_DIR } | prepend $env.FUNCTERM_SHIM_DIR) }'
        $'$env.config.hooks.display_output = { save_nushell_state ($cwd_file | to nuon) ($env_state_file | to nuon) ($config_state_file | to nuon) ($declaration_state_file | to nuon); $in | table }'
        '@NUSHELL_STATE_FUNCTIONS@'
    ] | str join (char newline) | save --force --raw $config_file
}
def --env restore_nushell_environment [
    env_state_file: path,
    config_state_file: path,
    previous_env_names: list<string>,
] {
    if ($env_state_file | path exists) {
        let saved_env = open --raw $env_state_file | from nuon
        for name in ($previous_env_names | where {|name| not ($name in ($saved_env | columns)) }) {
            hide-env $name
        }
        load-env $saved_env
    }
    if ($config_state_file | path exists) {
        $env.config = ($env.config | merge (open --raw $config_state_file | from nuon))
    }
}
" ;
const NUSHELL_STATE_FUNCTIONS: &str = "def save_nushell_state [
    cwd_file: path,
    env_state_file: path,
    config_state_file: path,
    declaration_state_file: path,
] {
    $env.PWD | save --force --raw $cwd_file
    $env
        | reject PWD config FUNCTERM_COMMAND_ID FUNCTERM_COMMAND_DIRECTORY
        | transpose name value
        | where {|item| not (($item.value | describe) starts-with 'closure') }
        | transpose --header-row --as-record
        | to nuon
        | save --force --raw $env_state_file
    $env.config | reject hooks
        | to nuon
        | save --force --raw $config_state_file
    let declarations = scope commands
        | where type == custom
        | where name not-in ['banner' 'pwd' 'save_nushell_state']
        | uniq-by name
        | each {|item| view source $item.name }
    let aliases = scope aliases
        | each {|item| $item | format pattern \"alias {name} = {expansion}\" }
    let source = $declarations | append $aliases
    if not ($source | is-empty) {
        $source | str join (char newline)
            | save --force --raw $declaration_state_file
    }
}";
