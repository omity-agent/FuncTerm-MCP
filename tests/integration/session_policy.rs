use crate::support::{
    locked_with_env, parse_command_result, parse_tab_created, run_cli, send_command, temp_dir,
};
#[test]
fn cli_profile_policy_is_per_tab_and_inherited_by_nested_shells() {
    let _guard = locked_with_env(&[]);
    let cwd = temp_dir("profile-policy");
    #[cfg(windows)]
    let (shell, query, nested) = (
        "powershell",
        "Write-Output $env:FUNCTERM_LOAD_PROFILE",
        "pwsh -NoLogo",
    );
    #[cfg(unix)]
    let (shell, query, nested) = (
        "bash",
        "printf '%s\\n' \"$FUNCTERM_LOAD_PROFILE\"",
        "bash -i",
    );
    for (no_profile, expected) in [(false, "1"), (true, "0")] {
        let mut arguments = vec![
            "new-tab",
            "--starting-directory",
            cwd.to_str().unwrap(),
            "--starting-shell",
            shell,
        ];
        if no_profile {
            arguments.push("--no-profile");
        }
        let output = run_cli(&arguments);
        assert!(output.status.success(), "{output:?}");
        let tab = parse_tab_created(&output);
        assert_policy(&tab.tab_id, query, expected);
        let switched = parse_command_result(&send_command(&tab.tab_id, nested, 10.0));
        assert!(switched.finished, "{}", switched.stderr);
        assert_policy(&tab.tab_id, query, expected);
    }
}
fn assert_policy(tab_id: &str, query: &str, expected: &str) {
    let result = parse_command_result(&send_command(tab_id, query, 10.0));
    assert!(result.finished);
    assert_eq!(result.exit_code, Some(0_i32), "{}", result.stderr);
    assert_eq!(result.stdout.trim(), expected);
}
#[test]
fn child_path_excludes_normal_and_elevated_runtime_shims() {
    let directory = tempfile::tempdir().unwrap();
    let runtime = directory.path().join("functerm");
    #[cfg(unix)]
    let runtime_root = runtime.join(format!("functerm-{}", rustix::process::geteuid().as_raw()));
    #[cfg(windows)]
    let runtime_root = runtime;
    let shims = runtime_root.join("services/fixture/generations/process/shims");
    let retained = [
        directory.path().join("tools"),
        directory.path().join("workspace/shims/normal"),
    ];
    let path = std::env::join_paths(
        [shims.join("normal"), shims.join("elevated")]
            .iter()
            .chain(retained.iter()),
    )
    .unwrap();
    let mut command = path_probe();
    crate::support::apply_test_environment(
        &mut command,
        &[("PATH".to_owned(), path.into_string().unwrap())],
    );
    let output = command.output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout).unwrap();
    let child_path = stdout
        .lines()
        .filter_map(|line| line.split_once('='))
        .find(|pair| pair.0.eq_ignore_ascii_case("PATH"))
        .unwrap()
        .1;
    let actual = std::env::split_paths(child_path).collect::<Vec<_>>();
    assert_eq!(actual, retained);
}
fn path_probe() -> std::process::Command {
    #[cfg(windows)]
    let mut command = {
        let root = std::env::var_os("SystemRoot").unwrap();
        let mut probe = std::process::Command::new(
            std::path::PathBuf::from(root)
                .join("System32")
                .join("cmd.exe"),
        );
        probe.args(["/D", "/C", "set", "PATH"]);
        probe
    };
    #[cfg(unix)]
    let mut command = {
        let mut probe = std::process::Command::new("/bin/sh");
        probe.args(["-c", "printf 'PATH=%s\\n' \"$PATH\""]);
        probe
    };
    command.stdin(std::process::Stdio::null());
    command
}
