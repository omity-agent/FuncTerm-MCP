use super::StartupContext;
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, COMMAND_INPUT_DIRECTORY,
    COMMAND_OUTPUT_DIRECTORY, COMMAND_WORKING_DIRECTORY_FILE, DISPATCH_FILE, DONE_FILE,
    HELPER_EXECUTABLE_ENV, SESSION_COMMANDS_DIRECTORY, SESSION_STATE_DIRECTORY, STDERR_FILE,
    STDOUT_FILE,
};
use anyhow::{Context as _, Result};
#[expect(
    clippy::uninlined_format_args,
    reason = "explicit aliases keep Rust format fields distinct from embedded Python templates"
)]
pub(super) fn script(context: StartupContext<'_>) -> Result<String> {
    let cwd = python_string(context.cwd)?;
    let ready = python_string(context.ready_file)?;
    let bootstrap = format ! (r#"import ast as @VAR_ast@
	import contextlib as @VAR_contextlib@
	import os as @VAR_os@
	import pathlib as @VAR_pathlib@
	import subprocess as @VAR_subprocess@
	import time as @VAR_time@
	@VAR_os@.chdir({cwd})
	@VAR_os@.environ["FUNCTERM_CURRENT_SHELL"] = "python"

	def @VAR_helper@(*@VAR_arguments@):
	    @VAR_helper_executable@ = @VAR_os@.environ["{helper_env}"]
	    @VAR_subprocess@.run([@VAR_helper_executable@, *@VAR_arguments@], check=True)

	def @VAR_prepend_shim@():
	    @VAR_shim@ = @VAR_os@.environ["FUNCTERM_SHIM_DIR"]
	    @VAR_path@ = @VAR_os@.environ.get("PATH", "")
	    @VAR_entries@ = [@VAR_entry@ for @VAR_entry@ in @VAR_path@.split(@VAR_os@.pathsep) if @VAR_entry@ and @VAR_entry@ != @VAR_shim@]
	    @VAR_os@.environ["PATH"] = @VAR_os@.pathsep.join([@VAR_shim@, *@VAR_entries@])

	def @VAR_execute@(@VAR_source@, @VAR_namespace@):
	    @VAR_tree@ = @VAR_ast@.parse(@VAR_source@, "<functerm>", "exec")
	    exec(compile(@VAR_ast@.Interactive(@VAR_tree@.body), "<functerm>", "single"), @VAR_namespace@, @VAR_namespace@)

	def _functerm_dispatch():
	    @VAR_root@ = @VAR_pathlib@.Path(@VAR_os@.environ["FUNCTERM_SESSION_ROOT"])
	    @VAR_dispatch@ = @VAR_root@ / "{state_directory}" / "{dispatch_file}"
	    @VAR_command_id@ = @VAR_dispatch@.read_text(encoding="utf-8")
	    @VAR_dispatch@.unlink()
	    @VAR_directory@ = @VAR_root@ / "{commands_directory}" / @VAR_command_id@
	    @VAR_input_directory@ = @VAR_directory@ / "{input_directory}"
	    @VAR_output_directory@ = @VAR_directory@ / "{output_directory}"
	    @VAR_source@ = (@VAR_input_directory@ / "{command_file}").read_text(encoding="utf-8")
	    @VAR_working_directory@ = (@VAR_input_directory@ / "{working_directory_file}").read_text(encoding="utf-8")
	    @VAR_stdout_file@ = @VAR_output_directory@ / "{stdout_file}"
	    @VAR_stderr_file@ = @VAR_output_directory@ / "{stderr_file}"
	    @VAR_done_file@ = @VAR_directory@ / "state" / "{done_file}"
	    @VAR_previous_id@ = @VAR_os@.environ.get("{command_id_env}")
	    @VAR_previous_directory@ = @VAR_os@.environ.get("{command_directory_env}")
	    @VAR_protected_environment@ = dict(@VAR_os@.environ)
	    @VAR_os@.environ["{command_id_env}"] = @VAR_command_id@
	    @VAR_os@.environ["{command_directory_env}"] = str(@VAR_directory@)
	    @VAR_exit_code@ = 0
	    @VAR_started@ = @VAR_time@.perf_counter()
	    try:
	        @VAR_helper@("internal-ensure-shims", "--directory", @VAR_os@.environ["FUNCTERM_SHIM_DIR"])
	        @VAR_prepend_shim@()
	        @VAR_os@.chdir(@VAR_working_directory@)
	        @VAR_helper@("internal-write-start", "--command-id", @VAR_command_id@, "--directory", str(@VAR_directory@))
	        with @VAR_stdout_file@.open("w", encoding="utf-8") as @VAR_stdout@:
	            with @VAR_stderr_file@.open("w", encoding="utf-8") as @VAR_stderr@:
	                with @VAR_contextlib@.redirect_stdout(@VAR_stdout@), @VAR_contextlib@.redirect_stderr(@VAR_stderr@):
	                    try:
	                        @VAR_execute@(@VAR_source@, globals())
	                    except SystemExit as @VAR_system_exit@:
	                        @VAR_system_exit_code@ = @VAR_system_exit@.code
	                        if @VAR_system_exit_code@ is None:
	                            @VAR_exit_code@ = 0
	                        elif isinstance(@VAR_system_exit_code@, int):
	                            @VAR_exit_code@ = @VAR_system_exit_code@
	                        else:
	                            @VAR_exit_code@ = 1
	                        raise
	                    except BaseException:
	                        @VAR_exit_code@ = 1
	                        import traceback as @VAR_traceback@
	                        @VAR_traceback@.print_exc()
	        print(@VAR_stdout_file@.read_text(encoding="utf-8"), end="")
	        print(@VAR_stderr_file@.read_text(encoding="utf-8"), end="", file=__import__("sys").stderr)
	    finally:
	        @VAR_environment_was_cleared@ = not @VAR_os@.environ
	        for @VAR_name@, @VAR_value@ in @VAR_protected_environment@.items():
	            if @VAR_environment_was_cleared@ or @VAR_name@.upper() in {{@PYTHON_PROTECTED_ENVIRONMENT@}}:
	                @VAR_os@.environ[@VAR_name@] = @VAR_value@
	        if not @VAR_done_file@.exists():
	            @VAR_elapsed@ = max(1, round((@VAR_time@.perf_counter() - @VAR_started@) * 1000))
	            @VAR_helper@(
	                "internal-write-done",
	                "--command-id", @VAR_command_id@,
	                "--exit-code", str(@VAR_exit_code@),
	                "--time-consumption", f"{{@VAR_elapsed@}}ms",
	                "--cwd", @VAR_os@.getcwd(),
	                "--directory", str(@VAR_directory@),
	            )
	        if @VAR_previous_id@ is None:
	            @VAR_os@.environ.pop("{command_id_env}", None)
	        else:
	            @VAR_os@.environ["{command_id_env}"] = @VAR_previous_id@
	        if @VAR_previous_directory@ is None:
	            @VAR_os@.environ.pop("{command_directory_env}", None)
	        else:
	            @VAR_os@.environ["{command_directory_env}"] = @VAR_previous_directory@
	@VAR_pathlib@.Path({ready}).touch()
	"# , helper_env = HELPER_EXECUTABLE_ENV , state_directory = SESSION_STATE_DIRECTORY , dispatch_file = DISPATCH_FILE , commands_directory = SESSION_COMMANDS_DIRECTORY , input_directory = COMMAND_INPUT_DIRECTORY , output_directory = COMMAND_OUTPUT_DIRECTORY , command_file = COMMAND_FILE , working_directory_file = COMMAND_WORKING_DIRECTORY_FILE , stdout_file = STDOUT_FILE , stderr_file = STDERR_FILE , done_file = DONE_FILE , command_id_env = COMMAND_ID_ENV , command_directory_env = COMMAND_DIRECTORY_ENV) . replace ("\n\t" , "\n") ;
    let protected = bootstrap.replace(
        "@PYTHON_PROTECTED_ENVIRONMENT@",
        &crate::shell::wrappers::quoted_protected_environment_names(),
    );
    Ok(crate::shell::wrappers::VariableNamespace::new().render(&protected))
}
fn python_string(path: &std::path::Path) -> Result<String> {
    sonic_rs::to_string(&crate::text::path_text(path, "Python bootstrap path")?)
        .context("failed to encode Python bootstrap path")
}
