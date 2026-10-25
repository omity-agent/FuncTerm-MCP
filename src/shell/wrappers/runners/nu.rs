mod capture_state;
mod instrumentation;
mod worker_setup;
pub(in crate::shell) use worker_setup::prepare_nushell_command;
pub(in crate::shell) fn wrapper() -> String {
    instrumentation::render(TEMPLATE)
}
const TEMPLATE : & str = "def --env @FUNCTION@ [@VAR_command_id@: string, @VAR_directory@: path, @VAR_working_directory@: path, @VAR_separate_output@: bool] {
		let @VAR_stdout_file@ = ($@VAR_directory@ | path join '@STDOUT@')
		let @VAR_stderr_file@ = ($@VAR_directory@ | path join '@STDERR@')
	    let @VAR_session_state_dir@ = ($env.FUNCTERM_SESSION_ROOT | path join 'state')
	    let @VAR_env_state_file@ = ($@VAR_session_state_dir@ | path join 'nushell-env.nuon')
	    let @VAR_config_state_file@ = ($@VAR_session_state_dir@ | path join 'nushell-config.nuon')
	    let @VAR_previous_env_names@ = if ($@VAR_env_state_file@ | path exists) {
	        open --raw $@VAR_env_state_file@ | from nuon | columns
	    } else {
	        []
	    }
		let @VAR_cwd_file@ = ($@VAR_directory@ | path join 'nushell-cwd.txt')
		let @VAR_exit_file@ = ($@VAR_directory@ | path join '@NUSHELL_EXIT_REQUEST@')
		let @VAR_config_file@ = ($@VAR_directory@ | path join 'functerm-config.nu')
		let @VAR_env_config_file@ = ($@VAR_directory@ | path join 'functerm-env.nu')
		let @VAR_launch_file@ = ($@VAR_directory@ | path join 'nushell-launch.txt')
	    let @VAR_previous_command_id@ = $env.@COMMAND_ID_ENV@?
	    let @VAR_previous_command_directory@ = $env.@COMMAND_DIR_ENV@?
	    $env.@COMMAND_ID_ENV@ = $@VAR_command_id@
	    $env.@COMMAND_DIR_ENV@ = ($@VAR_directory@ | path expand)
	    let @VAR_state@ = try {
	        let @VAR_shim_dir@ = $env.FUNCTERM_SHIM_DIR
	        $env.PATH = ($env.PATH | where {|@VAR_entry@| $@VAR_entry@ != $@VAR_shim_dir@ } | prepend $@VAR_shim_dir@)
	        let @VAR_command@ = (open --raw $@VAR_launch_file@)
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
	        mut @VAR_exit_code@ = 0
	        if $@VAR_separate_output@ {
	            let @VAR_result@ = (^$@VAR_nushell@ --no-history --config $@VAR_config_file@ --env-config $@VAR_env_config_file@ --commands $@VAR_command@ | complete)
	            $@VAR_exit_code@ = $@VAR_result@.exit_code
	            $@VAR_result@.stdout | save --force --raw $@VAR_stdout_file@
	            $@VAR_result@.stderr | save --force --raw $@VAR_stderr_file@
	            print --raw --no-newline (open --raw $@VAR_stdout_file@)
	            print --raw --no-newline --stderr (open --raw $@VAR_stderr_file@)
	        } else {
	            let @VAR_native_exit_code@ = try {
	                ^$@VAR_nushell@ --no-history --config $@VAR_config_file@ --env-config $@VAR_env_config_file@ --commands $@VAR_command@
	                0
	            } catch {|@VAR_native_error@|
	                if $@VAR_native_error@.exit_code? == null {
	                    error make $@VAR_native_error@
	                } else {
	                    $@VAR_native_error@.exit_code
	                }
	            }
	            $@VAR_exit_code@ = $@VAR_native_exit_code@
	        }
	        let @VAR_time_consumption@ = ((date now) - $@VAR_command_started_at@) | into string
	        let @VAR_command_cwd@ = if ($@VAR_cwd_file@ | path exists) {
	            open --raw $@VAR_cwd_file@
	        } else {
	            $@VAR_working_directory@ | path expand
	        }
	        restore_nushell_environment $@VAR_env_state_file@ $@VAR_config_state_file@ $@VAR_previous_env_names@
	        { cwd: $@VAR_command_cwd@, exit_code: $@VAR_exit_code@, time_consumption: $@VAR_time_consumption@ }
	    } catch {|@VAR_error@|
	        $@VAR_error@.msg | save --append --raw $@VAR_stderr_file@
	        print --stderr $@VAR_error@.msg
	        { cwd: ($@VAR_working_directory@ | path expand), exit_code: 1, time_consumption: '0ns' }
	    }
		    let @VAR_exit_request@ = if ($@VAR_exit_file@ | path exists) {
		        open --raw $@VAR_exit_file@ | from nuon
		    } else {
		        null
		    }
	    let @VAR_helper@ = $env.@HELPER_ENV@?
	    if ($@VAR_helper@ | is-empty) {
	        print --stderr '@HELPER_ENV@ is not set'
	        return 1
	    }
		    ^$@VAR_helper@ internal-write-done --command-id $@VAR_command_id@ --exit-code $@VAR_state@.exit_code --time-consumption $@VAR_state@.time_consumption --cwd $@VAR_state@.cwd --directory $@VAR_directory@
		    if $env.LAST_EXIT_CODE != 0 {
		        error make {msg: 'failed to publish command completion'}
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
		    if $@VAR_exit_request@ != null {
		        %exit $@VAR_exit_request@.exit_code --abort=$@VAR_exit_request@.abort
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
	def --env restore_nushell_environment [
	    @VAR_env_state_file@: path,
	    @VAR_config_state_file@: path,
	    @VAR_previous_env_names@: list<string>,
	] {
	    if ($@VAR_env_state_file@ | path exists) {
	        let @VAR_saved_env@ = open --raw $@VAR_env_state_file@ | from nuon
	        if not ($@VAR_saved_env@ | is-empty) {
	            for @VAR_name@ in ($@VAR_previous_env_names@ | where {|@VAR_name@| not ($@VAR_name@ in ($@VAR_saved_env@ | columns)) }) {
	                hide-env $@VAR_name@
	            }
	        }
	        load-env $@VAR_saved_env@
	    }
	    if ($@VAR_config_state_file@ | path exists) {
	        $env.config = ($env.config | merge (open --raw $@VAR_config_state_file@ | from nuon))
	    }
	}
	" ;
