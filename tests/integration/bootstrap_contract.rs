use crate::support::{
    locked_with_env, parse_command_result, parse_tab_created, required_executable, run_cli,
    send_command, temp_dir,
};
use functerm::shell::quote;
use portable_pty::CommandBuilder;
#[path = "terminal_fixture.rs"]
mod terminal_fixture;
use std::{
    fs,
    path::{Path, PathBuf},
};
#[test]
fn powershell_profile_functions_and_prompt_survive_protocol_repair() {
    let _guard = locked_with_env(&[]);
    let root = session_root("powershell", "Write-Output $env:FUNCTERM_SESSION_ROOT");
    let directory = prepare_command(
        &root,
        "command.ps1",
        "Profile-Function\nWrite-Output $env:FUNCTERM_LOAD_PROFILE\n",
    );
    let script = root.join("startup").join("profile-probe.ps1");
    let probe = format!(
        "function Profile-Function {{ 'PROFILE_FUNCTION_LOADED' }}
function prompt {{ 'CUSTOM_PROFILE_PROMPT> ' }}
Set-Alias -Name f -Value Write-Output
Set-PSReadLineOption -HistorySaveStyle SaveIncrementally
$env:FUNCTERM_SESSION_ROOT = 'broken'
$env:FUNCTERM_HELPER_EXECUTABLE = 'broken'
$env:FUNCTERM_LOAD_PROFILE = '1'
. {}
. f
prompt
Write-Output (Get-PSReadLineOption).HistorySaveStyle
",
        quote::powershell_path(&root.join("startup/powershell_init.ps1")).unwrap()
    );
    fs::write(&script, probe).unwrap();
    let mut command = CommandBuilder::new(required_executable(&["pwsh", "powershell"]));
    command.args([
        "-NoLogo",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
    ]);
    command.arg(script);
    let text = terminal_fixture::run(command);
    assert!(text.contains("CUSTOM_PROFILE_PROMPT"), "{text}");
    assert!(text.contains("SaveNothing"), "{text}");
    assert_result(&directory, "PROFILE_FUNCTION_LOADED\n0");
}
#[test]
fn nushell_profile_declarations_are_available_to_command_subprocesses() {
    let _guard = locked_with_env(&[]);
    let root = session_root("nu", "print $env.FUNCTERM_SESSION_ROOT");
    let directory = prepare_command(
        &root,
        "command.txt",
        "profile_alias\nprint $env.FUNCTERM_LOAD_PROFILE\n",
    );
    let profile = root.join("startup").join("fixture-config.nu");
    let env_config = root.join("startup").join("empty-env.nu");
    fs::write(&env_config, "").unwrap();
    fs :: write (& profile , "def profile_function [] { print 'PROFILE_FUNCTION_LOADED' }\nalias profile_alias = profile_function\nalias f = print 'BROKEN_DISPATCH'\n$env.FUNCTERM_SESSION_ROOT = 'broken'\n$env.FUNCTERM_HELPER_EXECUTABLE = 'broken'\n$env.FUNCTERM_LOAD_PROFILE = '1'\n") . unwrap () ;
    let executable = required_executable(&["nu"]);
    let mut command = CommandBuilder::new(executable);
    command.args(["--no-history", "--config"]);
    command.arg(profile);
    command.arg("--env-config");
    command.arg(env_config);
    command.arg("--commands");
    command.arg(format!(
        "source {}\nf",
        quote::nushell_path(&root.join("startup/nu_bootstrap.nu")).unwrap(),
    ));
    let captured = terminal_fixture::run(command);
    assert!(directory.join("state/done.json").exists(), "{captured}");
    assert_result(&directory, "PROFILE_FUNCTION_LOADED\n0");
}
#[test]
fn bash_startup_retains_profile_aliases_but_keeps_history_disabled() {
    let _guard = locked_with_env(&[]);
    let home = temp_dir("bash-profile-home");
    fs :: write (home . join (".bashrc") , "export PROFILE_MARKER=PROFILE_FUNCTION_LOADED\nprofile_function() { printf '%s\\n' \"$PROFILE_MARKER\"; }\nalias profile_alias=profile_function\nalias f='printf BROKEN_DISPATCH'\nexport FUNCTERM_SESSION_ROOT=broken\nexport FUNCTERM_HELPER_EXECUTABLE=broken\nexport FUNCTERM_LOAD_PROFILE=0\nHISTFILE=\"$HOME/.bash_history\"\nHISTSIZE=1000\nset -o history\nPS1='CUSTOM_PROFILE_PROMPT> '\n") . unwrap () ;
    let root = session_root_with_profile("bash", "printf '%s\\n' \"$FUNCTERM_SESSION_ROOT\"", true);
    let directory = prepare_command(
        &root,
        "command.txt",
        "profile_alias\nprintf '%s\\n' \"$FUNCTERM_LOAD_PROFILE\"\nbuiltin test -z \"${HISTFILE-}\"\n",
    );
    let mut command = CommandBuilder::new(required_executable(&["bash"]));
    command.args(["--noprofile", "--rcfile"]);
    command.arg(root.join("startup/bash_init.sh"));
    command.args(["-i", "-c", "f"]);
    command.env("HOME", home.to_str().unwrap().replace('\\', "/"));
    let captured = terminal_fixture::run(command);
    assert!(directory.join("state/done.json").exists(), "{captured}");
    assert_result(&directory, "PROFILE_FUNCTION_LOADED\n1");
}
#[test]
fn python_startup_preserves_functions_and_repairs_managed_environment() {
    let _guard = locked_with_env(&[]);
    let root = session_root_with_profile(
        "python",
        "import os; print(os.environ['FUNCTERM_SESSION_ROOT'])",
        true,
    );
    let directory = prepare_command(
        &root,
        "command.txt",
        "profile_function()\nprint(__import__('os').environ['FUNCTERM_LOAD_PROFILE'])\n",
    );
    let profile = root.join("startup").join("interactive-profile.py");
    fs :: write (& profile , "import os\ndef profile_function():\n    print('PROFILE_FUNCTION_LOADED')\ndef _functerm_dispatch():\n    print('BROKEN_DISPATCH')\nos.environ['FUNCTERM_SESSION_ROOT'] = 'broken'\nos.environ['FUNCTERM_HELPER_EXECUTABLE'] = 'broken'\nos.environ['FUNCTERM_LOAD_PROFILE'] = '0'\n") . unwrap () ;
    let probe = root.join("startup").join("dispatch-probe.py");
    let bootstrap =
        sonic_rs::to_string(root.join("startup/python_repl.py").to_str().unwrap()).unwrap();
    fs :: write (& probe , format ! ("exec(compile(open({bootstrap}, 'rb').read(), {bootstrap}, 'exec'), globals())\n_functerm_dispatch()\n")) . unwrap () ;
    let mut command = CommandBuilder::new(required_executable(&["python3", "python"]));
    command.arg("-u");
    command.arg(probe);
    command.env("PYTHONSTARTUP", profile);
    let captured = terminal_fixture::run(command);
    assert!(directory.join("state/done.json").exists(), "{captured}");
    assert_result(&directory, "PROFILE_FUNCTION_LOADED\n1");
}
fn session_root(shell: &str, query: &str) -> PathBuf {
    session_root_with_profile(shell, query, false)
}
fn session_root_with_profile(shell: &str, query: &str, load_profile: bool) -> PathBuf {
    let cwd = temp_dir("bootstrap-contract");
    let mut arguments = vec![
        "new-tab",
        "--starting-shell",
        shell,
        "--starting-directory",
        cwd.to_str().unwrap(),
    ];
    if !load_profile {
        arguments.push("--no-profile");
    }
    let tab = parse_tab_created(&run_cli(&arguments));
    let result = parse_command_result(&send_command(&tab.tab_id, query, 10.0));
    assert_eq!(result.exit_code, Some(0_i32), "{}", result.stderr);
    PathBuf::from(result.stdout.trim())
}
fn prepare_command(root: &Path, script_name: &str, script: &str) -> PathBuf {
    let directory = root.join("commands").join("profile-probe-command");
    for child in ["input", "output", "state"] {
        fs::create_dir_all(directory.join(child)).unwrap();
    }
    fs::write(directory.join("input").join(script_name), script).unwrap();
    fs::write(directory.join("input/cwd.txt"), root.to_str().unwrap()).unwrap();
    fs::write(root.join("state/dispatch"), "profile-probe-command").unwrap();
    directory
}
fn assert_result(directory: &Path, expected: &str) {
    let done = fs::read_to_string(directory.join("state/done.json")).unwrap();
    let record: rmcp::serde_json::Value = sonic_rs::from_str(&done).unwrap();
    assert_eq!(
        record.get("exit_code"),
        Some(&rmcp::serde_json::json!(0_i32)),
        "{done}"
    );
    let stdout = fs::read_to_string(directory.join("output/stdout.txt")).unwrap();
    assert_eq!(stdout.replace('\r', "").trim(), expected);
    let stderr = fs::read_to_string(directory.join("output/stderr.txt")).unwrap();
    assert!(stderr.trim().is_empty(), "{stderr}");
}
