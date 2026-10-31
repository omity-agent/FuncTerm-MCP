use super::StartupContext;
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, HELPER_EXECUTABLE_ENV, SESSION_EXIT_FILE,
    STDERR_FILE, STDOUT_FILE,
};
use anyhow::{Context as _, Result};
pub(super) fn script(context: StartupContext<'_>) -> Result<String> {
    let cwd = python_string(context.cwd)?;
    let ready = python_string(context.ready_file)?;
    let profile = if context.load_profile {
        r#"@VAR_startup_file@ = @VAR_os@.environ.get("PYTHONSTARTUP")
if @VAR_startup_file@:
    with open(@VAR_startup_file@, "rb") as @VAR_startup_source@:
        exec(compile(@VAR_startup_source@.read(), @VAR_startup_file@, "exec"), globals())
"#
    } else {
        ""
    };
    let protected_environment = crate::shell::wrappers::startup_environment(
        crate::shell::ShellChoice::Python,
        context.environment,
    )?;
    let style = if context.ignore_shell_style {
        "@VAR_os@.environ[\"NO_COLOR\"] = \"1\"\n@VAR_os@.environ[\"PYTHON_COLORS\"] = \"0\"\n@VAR_sys@.ps1 = \">>> \"\n@VAR_sys@.ps2 = \"... \"\n"
    } else {
        ""
    };
    let bootstrap = format!(
        r#"import ast as @VAR_ast@
import codeop as @VAR_codeop@
import code as @VAR_console@
import contextlib as @VAR_contextlib@
import json as @VAR_json@
import os as @VAR_os@
import pathlib as @VAR_pathlib@
import subprocess as @VAR_subprocess@
import sys as @VAR_sys@
import time as @VAR_time@
{profile}{protected_environment}{style}
@VAR_os@.chdir({cwd})
@VAR_os@.environ["FUNCTERM_CURRENT_SHELL"] = "python"
if @VAR_os@.name == "nt":
    import ctypes as @VAR_ctypes@
    import msvcrt as @VAR_msvcrt@
    @VAR_set_std_handle@ = @VAR_ctypes@.WinDLL("kernel32", use_last_error=True).SetStdHandle
    @VAR_set_std_handle@.argtypes = [@VAR_ctypes@.c_uint32, @VAR_ctypes@.c_void_p]
    @VAR_set_std_handle@.restype = @VAR_ctypes@.c_int
def @VAR_sync_handles@():
    if @VAR_os@.name == "nt":
        for @VAR_descriptor@, @VAR_standard@ in [(1, -11), (2, -12)]:
            if not @VAR_set_std_handle@(@VAR_standard@, @VAR_msvcrt@.get_osfhandle(@VAR_descriptor@)):
                raise @VAR_ctypes@.WinError(@VAR_ctypes@.get_last_error())
def @VAR_helper@(*@VAR_arguments@, capture=False):
    @VAR_helper_executable@ = @VAR_os@.environ["{HELPER_EXECUTABLE_ENV}"]
    return @VAR_subprocess@.run(
        [@VAR_helper_executable@, *@VAR_arguments@], check=True,
        capture_output=capture, text=capture, encoding="utf-8" if capture else None,
    ).stdout
def @VAR_prepend_shim@():
    @VAR_shim@ = @VAR_os@.environ["FUNCTERM_SHIM_DIR"]
    @VAR_path@ = @VAR_os@.environ.get("PATH", "")
    @VAR_entries@ = [@VAR_entry@ for @VAR_entry@ in @VAR_path@.split(@VAR_os@.pathsep) if @VAR_entry@ and @VAR_entry@ != @VAR_shim@]
    @VAR_os@.environ["PATH"] = @VAR_os@.pathsep.join([@VAR_shim@, *@VAR_entries@])
@VAR_compiler@ = @VAR_codeop@.Compile()
def @VAR_execute@(@VAR_source@, @VAR_namespace@):
    @VAR_tree@ = @VAR_ast@.parse(@VAR_source@, "<functerm>", "exec")
    @VAR_code@ = @VAR_compiler@(@VAR_ast@.Interactive(@VAR_tree@.body), "<functerm>", "single")
    exec(@VAR_code@, @VAR_namespace@, @VAR_namespace@)
@@VAR_contextlib@.contextmanager
def @VAR_redirect_output@(@VAR_stdout_file@, @VAR_stderr_file@):
    @VAR_sys@.stdout.flush()
    @VAR_sys@.stderr.flush()
    @VAR_saved_stdout@ = @VAR_os@.dup(1)
    @VAR_saved_stderr@ = @VAR_os@.dup(2)
    try:
        with @VAR_stdout_file@.open("w", encoding="utf-8") as @VAR_stdout@, @VAR_stderr_file@.open("w", encoding="utf-8") as @VAR_stderr@:
            @VAR_os@.dup2(@VAR_stdout@.fileno(), 1)
            @VAR_os@.dup2(@VAR_stderr@.fileno(), 2)
            @VAR_sync_handles@()
            with @VAR_contextlib@.redirect_stdout(@VAR_stdout@), @VAR_contextlib@.redirect_stderr(@VAR_stderr@):
                yield
            @VAR_stdout@.flush()
            @VAR_stderr@.flush()
    finally:
        @VAR_os@.dup2(@VAR_saved_stdout@, 1)
        @VAR_os@.dup2(@VAR_saved_stderr@, 2)
        @VAR_sync_handles@()
        @VAR_os@.close(@VAR_saved_stdout@)
        @VAR_os@.close(@VAR_saved_stderr@)
def _functerm_dispatch():
    @VAR_receipt@ = @VAR_json@.loads(@VAR_helper@(
        "internal-claim-command", "--session-root",
        @VAR_os@.environ["FUNCTERM_SESSION_ROOT"], "--shell", "python", capture=True,
    ))
    @VAR_command_id@ = @VAR_receipt@["command_id"]
    @VAR_directory@ = @VAR_pathlib@.Path(@VAR_receipt@["directory"])
    @VAR_source@ = (@VAR_directory@ / "{COMMAND_FILE}").read_text(encoding="utf-8")
    @VAR_working_directory@ = @VAR_receipt@["working_directory"]
    @VAR_stdout_file@ = @VAR_directory@ / "{STDOUT_FILE}"
    @VAR_stderr_file@ = @VAR_directory@ / "{STDERR_FILE}"
    @VAR_previous_id@ = @VAR_os@.environ.get("{COMMAND_ID_ENV}")
    @VAR_previous_directory@ = @VAR_os@.environ.get("{COMMAND_DIRECTORY_ENV}")
    @VAR_protected_environment@ = dict(@VAR_os@.environ)
    @VAR_os@.environ["{COMMAND_ID_ENV}"] = @VAR_command_id@
    @VAR_os@.environ["{COMMAND_DIRECTORY_ENV}"] = str(@VAR_directory@)
    @VAR_exit_code@ = 0
    @VAR_session_exit@ = False
    @VAR_started@ = @VAR_time@.perf_counter()
    try:
        @VAR_prepend_shim@()
        @VAR_os@.chdir(@VAR_working_directory@)
        @VAR_helper@("internal-write-start", "--command-id", @VAR_command_id@, "--directory", str(@VAR_directory@))
        with @VAR_contextlib@.ExitStack() as @VAR_output_context@:
            if @VAR_receipt@["separate_output"]:
                @VAR_output_context@.enter_context(@VAR_redirect_output@(@VAR_stdout_file@, @VAR_stderr_file@))
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
                    print(@VAR_system_exit_code@, file=@VAR_sys@.stderr)
                @VAR_session_exit@ = True
            except KeyboardInterrupt:
                @VAR_exit_code@ = 130
            except BaseException:
                @VAR_exit_code@ = 1
                import traceback as @VAR_traceback@
                @VAR_traceback@.print_exc()
        if @VAR_receipt@["separate_output"]:
            print(@VAR_stdout_file@.read_text(encoding="utf-8"), end="")
            print(@VAR_stderr_file@.read_text(encoding="utf-8"), end="", file=@VAR_sys@.stderr)
    finally:
        @VAR_environment_was_cleared@ = not @VAR_os@.environ
        for @VAR_name@, @VAR_value@ in @VAR_protected_environment@.items():
            if @VAR_environment_was_cleared@ or @VAR_name@.upper() in {{@PYTHON_PROTECTED_ENVIRONMENT@}}:
                @VAR_os@.environ[@VAR_name@] = @VAR_value@
        @VAR_prepend_shim@()
        @VAR_elapsed@ = max(1, round((@VAR_time@.perf_counter() - @VAR_started@) * 1000))
        if @VAR_session_exit@ and not @VAR_os@.environ.get("FUNCTERM_PARENT_SHELL"):
            (@VAR_directory@ / "{SESSION_EXIT_FILE}").touch()
        @VAR_helper@(
            "internal-write-done",
            "--command-id", @VAR_command_id@,
            "--exit-code", str(@VAR_exit_code@),
            "--time-consumption", f"{{@VAR_elapsed@}}ms",
            "--cwd", @VAR_os@.getcwd(),
            "--directory", str(@VAR_directory@),
        )
        if @VAR_previous_id@ is None:
            @VAR_os@.environ.pop("{COMMAND_ID_ENV}", None)
        else:
            @VAR_os@.environ["{COMMAND_ID_ENV}"] = @VAR_previous_id@
        if @VAR_previous_directory@ is None:
            @VAR_os@.environ.pop("{COMMAND_DIRECTORY_ENV}", None)
        else:
            @VAR_os@.environ["{COMMAND_DIRECTORY_ENV}"] = @VAR_previous_directory@
    if @VAR_session_exit@:
        @VAR_sys@.stdout.flush()
        @VAR_sys@.stderr.flush()
        raise SystemExit(@VAR_exit_code@)
@VAR_pathlib@.Path({ready}).touch()
def @VAR_readline@(@VAR_prompt@):
    @VAR_sys@.stdout.write(@VAR_prompt@)
    @VAR_sys@.stdout.flush()
    @VAR_line@ = @VAR_sys@.stdin.readline()
    if not @VAR_line@:
        raise EOFError
    return @VAR_line@.rstrip("\r\n")
@VAR_console@.interact(banner="", readfunc=@VAR_readline@, exitmsg="", local=globals())
"#
    );
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
