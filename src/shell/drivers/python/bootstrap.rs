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
    Ok(format!(
        r#"import ast as _functerm_ast
import contextlib as _functerm_contextlib
import os as _functerm_os
import pathlib as _functerm_pathlib
import subprocess as _functerm_subprocess
import time as _functerm_time

_functerm_os.chdir({cwd})
_functerm_os.environ["FUNCTERM_CURRENT_SHELL"] = "python"

def _functerm_helper(*arguments):
    helper = _functerm_os.environ["{helper_env}"]
    _functerm_subprocess.run([helper, *arguments], check=True)

def _functerm_prepend_shim():
    shim = _functerm_os.environ["FUNCTERM_SHIM_DIR"]
    path = _functerm_os.environ.get("PATH", "")
    entries = [entry for entry in path.split(_functerm_os.pathsep) if entry and entry != shim]
    _functerm_os.environ["PATH"] = _functerm_os.pathsep.join([shim, *entries])

def _functerm_execute(source, namespace):
    tree = _functerm_ast.parse(source, "<functerm>", "exec")
    exec(compile(_functerm_ast.Interactive(tree.body), "<functerm>", "single"), namespace, namespace)

def _functerm_dispatch():
    root = _functerm_pathlib.Path(_functerm_os.environ["FUNCTERM_SESSION_ROOT"])
    dispatch = root / "{state_directory}" / "{dispatch_file}"
    command_id = dispatch.read_text(encoding="utf-8")
    dispatch.unlink()
    directory = root / "{commands_directory}" / command_id
    input_directory = directory / "{input_directory}"
    output_directory = directory / "{output_directory}"
    source = (input_directory / "{command_file}").read_text(encoding="utf-8")
    working_directory = (input_directory / "{working_directory_file}").read_text(encoding="utf-8")
    stdout_file = output_directory / "{stdout_file}"
    stderr_file = output_directory / "{stderr_file}"
    done_file = directory / "state" / "{done_file}"
    previous_id = _functerm_os.environ.get("{command_id_env}")
    previous_directory = _functerm_os.environ.get("{command_directory_env}")
    _functerm_os.environ["{command_id_env}"] = command_id
    _functerm_os.environ["{command_directory_env}"] = str(directory)
    exit_code = 0
    started = _functerm_time.perf_counter()
    try:
        _functerm_helper("internal-ensure-shims", "--directory", _functerm_os.environ["FUNCTERM_SHIM_DIR"])
        _functerm_prepend_shim()
        _functerm_os.chdir(working_directory)
        _functerm_helper("internal-write-start", "--command-id", command_id, "--directory", str(directory))
        with stdout_file.open("w", encoding="utf-8") as stdout:
            with stderr_file.open("w", encoding="utf-8") as stderr:
                with _functerm_contextlib.redirect_stdout(stdout), _functerm_contextlib.redirect_stderr(stderr):
                    try:
                        _functerm_execute(source, globals())
                    except SystemExit:
                        raise
                    except BaseException:
                        exit_code = 1
                        import traceback as _functerm_traceback
                        _functerm_traceback.print_exc()
        print(stdout_file.read_text(encoding="utf-8"), end="")
        print(stderr_file.read_text(encoding="utf-8"), end="", file=__import__("sys").stderr)
    finally:
        if not done_file.exists():
            elapsed = max(1, round((_functerm_time.perf_counter() - started) * 1000))
            _functerm_helper(
                "internal-write-done",
                "--command-id", command_id,
                "--exit-code", str(exit_code),
                "--time-consumption", f"{{elapsed}}ms",
                "--cwd", _functerm_os.getcwd(),
                "--directory", str(directory),
            )
        if previous_id is None:
            _functerm_os.environ.pop("{command_id_env}", None)
        else:
            _functerm_os.environ["{command_id_env}"] = previous_id
        if previous_directory is None:
            _functerm_os.environ.pop("{command_directory_env}", None)
        else:
            _functerm_os.environ["{command_directory_env}"] = previous_directory

_functerm_pathlib.Path({ready}).touch()
"#,
        helper_env = HELPER_EXECUTABLE_ENV,
        state_directory = SESSION_STATE_DIRECTORY,
        dispatch_file = DISPATCH_FILE,
        commands_directory = SESSION_COMMANDS_DIRECTORY,
        input_directory = COMMAND_INPUT_DIRECTORY,
        output_directory = COMMAND_OUTPUT_DIRECTORY,
        command_file = COMMAND_FILE,
        working_directory_file = COMMAND_WORKING_DIRECTORY_FILE,
        stdout_file = STDOUT_FILE,
        stderr_file = STDERR_FILE,
        done_file = DONE_FILE,
        command_id_env = COMMAND_ID_ENV,
        command_directory_env = COMMAND_DIRECTORY_ENV,
    ))
}
fn python_string(path: &std::path::Path) -> Result<String> {
    sonic_rs::to_string(&crate::text::path_text(path, "Python bootstrap path")?)
        .context("failed to encode Python bootstrap path")
}
