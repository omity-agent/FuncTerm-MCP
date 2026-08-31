use super::template;
use crate::contract::POSIX_COMMAND_FUNCTION;
pub(in crate::shell) fn wrapper() -> String {
    let stateful = TEMPLATE.replace("@NUSHELL_STATE_FUNCTIONS@", NUSHELL_STATE_FUNCTIONS);
    let protected = stateful.replace(
        "@NUSHELL_PROTECTED_ENVIRONMENT@",
        &super::variables::nushell_protected_environment_names(),
    );
    let runner = template::render_command_function(&protected, POSIX_COMMAND_FUNCTION);
    let wrapper = format!("{runner}\n{}", super::template::nushell_dispatcher());
    super::VariableNamespace::new().render(&wrapper)
}
const TEMPLATE : & str = "def --env @FUNCTION@ [@VAR_command_id@: string, @VAR_directory@: path, @VAR_working_directory@: path] {
	    let @VAR_input_dir@ = ($@VAR_directory@ | path join '@INPUT_DIR@')
	    let @VAR_output_dir@ = ($@VAR_directory@ | path join '@OUTPUT_DIR@')
	    let @VAR_state_dir@ = ($@VAR_directory@ | path join '@STATE_DIR@')
	    mkdir $@VAR_input_dir@ $@VAR_output_dir@ $@VAR_state_dir@
	    let @VAR_stdout_file@ = ($@VAR_output_dir@ | path join '@STDOUT@')
	    let @VAR_stderr_file@ = ($@VAR_output_dir@ | path join '@STDERR@')
	    let @VAR_command_file@ = ($@VAR_input_dir@ | path join '@COMMAND@')
	    let @VAR_done_file@ = ($@VAR_state_dir@ | path join '@DONE@')
	    let @VAR_session_state_dir@ = ($env.FUNCTERM_SESSION_ROOT | path join 'state')
	    let @VAR_env_state_file@ = ($@VAR_session_state_dir@ | path join 'nushell-env.nuon')
	    let @VAR_config_state_file@ = ($@VAR_session_state_dir@ | path join 'nushell-config.nuon')
	    let @VAR_declaration_state_file@ = ($@VAR_session_state_dir@ | path join 'nushell-declarations.nu')
	    mkdir $@VAR_session_state_dir@
	    if not ($@VAR_declaration_state_file@ | path exists) {
	        $'' | save --force --raw $@VAR_declaration_state_file@
	    }
	    let @VAR_previous_env_names@ = if ($@VAR_env_state_file@ | path exists) {
	        open --raw $@VAR_env_state_file@ | from nuon | columns
	    } else {
	        []
	    }
	    let @VAR_cwd_file@ = ($@VAR_state_dir@ | path join 'nushell-cwd.txt')
	    let @VAR_config_file@ = ($@VAR_input_dir@ | path join 'functerm-config.nu')
	    let @VAR_env_config_file@ = ($@VAR_input_dir@ | path join 'functerm-env.nu')
	    let @VAR_script_file@ = ($@VAR_input_dir@ | path join 'command.nu')
	    let @VAR_previous_command_id@ = $env.@COMMAND_ID_ENV@?
	    let @VAR_previous_command_directory@ = $env.@COMMAND_DIR_ENV@?
	    $env.@COMMAND_ID_ENV@ = $@VAR_command_id@
	    $env.@COMMAND_DIR_ENV@ = ($@VAR_directory@ | path expand)
	    let @VAR_state@ = try {
	        ensure_nushell_shims
	        open --raw $@VAR_command_file@ | save --force --raw $@VAR_script_file@
	        rm --force $@VAR_command_file@
	        write_nushell_config $@VAR_config_file@ $@VAR_cwd_file@ $@VAR_env_state_file@ $@VAR_config_state_file@ $@VAR_declaration_state_file@
	        $'' | save --force --raw $@VAR_env_config_file@
	        let @VAR_command@ = $'source (char single_quote)($@VAR_declaration_state_file@)(char single_quote); source (char single_quote)($@VAR_script_file@)(char single_quote)'
	        let @VAR_nushell@ = $env.FUNCTERM_REAL_NUSHELL?
	        if ($@VAR_nushell@ | is-empty) {
	            error make {msg: 'FUNCTERM_REAL_NUSHELL is not set'}
	        }
	        let @VAR_helper@ = $env.@HELPER_ENV@?
	        if ($@VAR_helper@ | is-empty) {
	            error make {msg: '@HELPER_ENV@ is not set'}
	        }
	        ^$@VAR_helper@ internal-write-start --command-id $@VAR_command_id@ --directory $@VAR_directory@
	        if $env.LAST_EXIT_CODE != 0 {
	            error make {msg: 'failed to publish command start'}
	        }
	        let @VAR_command_started_at@ = date now
	        let @VAR_result@ = (^$@VAR_nushell@ --no-history --config $@VAR_config_file@ --env-config $@VAR_env_config_file@ --commands $@VAR_command@ | complete)
	        let @VAR_time_consumption@ = ((date now) - $@VAR_command_started_at@) | into string
	        $@VAR_result@.stdout | save --force --raw $@VAR_stdout_file@
	        $@VAR_result@.stderr | save --force --raw $@VAR_stderr_file@
	        if ($@VAR_stdout_file@ | path exists) {
	            print --raw --no-newline (open --raw $@VAR_stdout_file@)
	        }
	        if ($@VAR_stderr_file@ | path exists) {
	            print --raw --no-newline --stderr (open --raw $@VAR_stderr_file@)
	        }
	        rm --force $@VAR_script_file@ $@VAR_config_file@ $@VAR_env_config_file@
	        let @VAR_command_cwd@ = if ($@VAR_cwd_file@ | path exists) {
	            open --raw $@VAR_cwd_file@
	        } else {
	            $@VAR_working_directory@ | path expand
	        }
	        rm --force $@VAR_cwd_file@
	        restore_nushell_environment $@VAR_env_state_file@ $@VAR_config_state_file@ $@VAR_previous_env_names@
	        { cwd: $@VAR_command_cwd@, exit_code: $@VAR_result@.exit_code, time_consumption: $@VAR_time_consumption@ }
	    } catch {|@VAR_error@|
	        $@VAR_error@.msg | save --append --raw $@VAR_stderr_file@
	        print --stderr $@VAR_error@.msg
	        { cwd: ($@VAR_working_directory@ | path expand), exit_code: 1, time_consumption: '0ns' }
	    }
	    mkdir $@VAR_state_dir@
	    if not ($@VAR_done_file@ | path exists) {
	        let @VAR_helper@ = $env.@HELPER_ENV@?
	        if ($@VAR_helper@ | is-empty) {
	            print --stderr '@HELPER_ENV@ is not set'
	            return 1
	        }
	        ^$@VAR_helper@ internal-write-done --command-id $@VAR_command_id@ --exit-code $@VAR_state@.exit_code --time-consumption $@VAR_state@.time_consumption --cwd $@VAR_state@.cwd --directory $@VAR_directory@
	    }
	    if ($@VAR_previous_command_id@ | is-empty) {
	        hide-env @COMMAND_ID_ENV@
	    } else {
	        $env.@COMMAND_ID_ENV@ = $@VAR_previous_command_id@
	    }
	    if ($@VAR_previous_command_directory@ | is-empty) {
	        hide-env @COMMAND_DIR_ENV@
	    } else {
	        $env.@COMMAND_DIR_ENV@ = $@VAR_previous_command_directory@
	    }
	}
	def --env ensure_nushell_shims [] {
	    let @VAR_shim_dir@ = $env.FUNCTERM_SHIM_DIR?
	    if not ($@VAR_shim_dir@ | is-empty) {
	        let @VAR_helper@ = $env.@HELPER_ENV@?
	        if ($@VAR_helper@ | is-empty) {
	            error make {msg: '@HELPER_ENV@ is not set'}
	        }
	        ^$@VAR_helper@ internal-ensure-shims --directory $@VAR_shim_dir@
        if $env.LAST_EXIT_CODE != 0 {
            error make {msg: 'failed to ensure FuncTerm shell shims'}
        }
	        $env.PATH = ($env.PATH | where {|@VAR_entry@| $@VAR_entry@ != $@VAR_shim_dir@ } | prepend $@VAR_shim_dir@)
    }
	}
	def write_nushell_config [
	    @VAR_config_file@: path,
	    @VAR_cwd_file@: path,
	    @VAR_env_state_file@: path,
	    @VAR_config_state_file@: path,
	    @VAR_declaration_state_file@: path,
] {
    [
        '$env.config.show_banner = false'
        '$env.config.use_ansi_coloring = false'
        '$env.config.shell_integration = { osc2: false, osc7: false, osc8: false, osc9_9: false, osc133: false, osc633: false, reset_application_mode: false }'
        '$env.config.history.file_format = \"plaintext\"'
        '$env.config.history.max_size = 0'
        '$env.config.history.sync_on_enter = false'
	        $'let @VAR_functerm_env_state_file@ = ($@VAR_env_state_file@ | to nuon)'
	        'if ($@VAR_functerm_env_state_file@ | path exists) { load-env (open --raw $@VAR_functerm_env_state_file@ | from nuon) }'
	        $'let @VAR_functerm_config_state_file@ = ($@VAR_config_state_file@ | to nuon)'
	        'if ($@VAR_functerm_config_state_file@ | path exists) { $env.config = ($env.config | merge (open --raw $@VAR_functerm_config_state_file@ | from nuon)) }'
	        'if not ($env.FUNCTERM_SHIM_DIR? | is-empty) { $env.PATH = ($env.PATH | where {|@VAR_entry@| $@VAR_entry@ != $env.FUNCTERM_SHIM_DIR } | prepend $env.FUNCTERM_SHIM_DIR) }'
	        $'$env.config.hooks.display_output = { save_nushell_state ($@VAR_cwd_file@ | to nuon) ($@VAR_env_state_file@ | to nuon) ($@VAR_config_state_file@ | to nuon) ($@VAR_declaration_state_file@ | to nuon); $in | table }'
	        '@NUSHELL_STATE_FUNCTIONS@'
	    ] | str join (char newline) | save --force --raw $@VAR_config_file@
	}
	def --env restore_nushell_environment [
	    @VAR_env_state_file@: path,
	    @VAR_config_state_file@: path,
	    @VAR_previous_env_names@: list<string>,
	] {
	    if ($@VAR_env_state_file@ | path exists) {
	        let @VAR_saved_env@ = open --raw $@VAR_env_state_file@ | from nuon
	        for @VAR_name@ in ($@VAR_previous_env_names@ | where {|@VAR_name@| not ($@VAR_name@ in ($@VAR_saved_env@ | columns)) }) {
	            hide-env $@VAR_name@
	        }
	        load-env $@VAR_saved_env@
	    }
	    if ($@VAR_config_state_file@ | path exists) {
	        $env.config = ($env.config | merge (open --raw $@VAR_config_state_file@ | from nuon))
	    }
	}
	" ;
const NUSHELL_STATE_FUNCTIONS: &str = "def save_nushell_state [
	    @VAR_cwd_file@: path,
	    @VAR_env_state_file@: path,
	    @VAR_config_state_file@: path,
	    @VAR_declaration_state_file@: path,
	] {
	    $env.PWD | save --force --raw $@VAR_cwd_file@
	    let @VAR_environment_entries@ = $env
	        | reject --optional PWD config @NUSHELL_PROTECTED_ENVIRONMENT@
	        | transpose @VAR_name@ @VAR_value@
	        | where {|@VAR_item@| not (($@VAR_item@.@VAR_value@ | describe) starts-with 'closure') }
	    let @VAR_saved_environment@ = if ($@VAR_environment_entries@ | is-empty) {
	        {}
	    } else {
	        $@VAR_environment_entries@ | transpose --header-row --as-record
	    }
	    $@VAR_saved_environment@
	        | to nuon
	        | save --force --raw $@VAR_env_state_file@
	    let @VAR_saved_config@ = $env.config? | default {}
	    $@VAR_saved_config@ | reject --optional hooks
	        | to nuon
	        | save --force --raw $@VAR_config_state_file@
	    let @VAR_declarations@ = scope commands
	        | where type == custom
	        | where name not-in ['banner' 'pwd' 'save_nushell_state']
	        | uniq-by name
	        | each {|@VAR_item@| view source $@VAR_item@.name }
	    let @VAR_aliases@ = scope aliases
	        | each {|@VAR_item@| $@VAR_item@ | format pattern \"alias {name} = {expansion}\" }
	    let @VAR_source@ = $@VAR_declarations@ | append $@VAR_aliases@
	    if not ($@VAR_source@ | is-empty) {
	        $@VAR_source@ | str join (char newline)
	            | save --force --raw $@VAR_declaration_state_file@
    }
}";
