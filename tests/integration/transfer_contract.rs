use crate::support::{locked_with_env, run_cli_with_env, temp_dir};
#[cfg(windows)]
use crate::terminal_fixture;
use rmcp::serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Output;
#[test]
fn command_claims_preserve_data_and_are_consumed_once() {
    let guard = locked_with_env(&[]);
    for shell in ["powershell", "nu", "python", "bun", "bash", "zsh", "cmd"] {
        let root = prepare_root("receipt-encoding");
        let expected_cwd = "路径 ' & %variable% !value! $()";
        let directory = prepare_record(&root, expected_cwd);
        let output = claim(&root, shell, &guard);
        assert!(output.status.success(), "{output:?}");
        let fields = match shell {
            "bash" | "zsh" => String::from_utf8(output.stdout)
                .unwrap()
                .split('\0')
                .take(3)
                .map(str::to_owned)
                .collect::<Vec<_>>(),
            "cmd" => String::from_utf8(output.stdout)
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect(),
            _ => {
                let receipt: Value = sonic_rs::from_slice(&output.stdout).unwrap();
                ["command_id", "directory", "working_directory"]
                    .map(|field| receipt.get(field).unwrap().as_str().unwrap().to_owned())
                    .to_vec()
            }
        };
        assert_eq!(
            fields,
            ["receipt-command", directory.to_str().unwrap(), expected_cwd]
        );
        assert!(!root.join("state/dispatch").exists());
        let repeated = claim(&root, shell, &guard);
        assert!(!repeated.status.success());
        assert!(
            String::from_utf8(repeated.stderr)
                .unwrap()
                .contains("failed to claim pending command")
        );
        if shell == "nu" {
            let config = fs::read_to_string(directory.join("input/functerm-config.nu")).unwrap();
            assert!(config.contains("save_nushell_state"));
            assert!(!config.contains("@VAR_"));
            let launch = fs::read_to_string(directory.join("input/nushell-launch.txt")).unwrap();
            assert!(
                launch.contains(
                    &functerm::shell::quote::nushell_path(
                        &directory.join("input").join("command.nu")
                    )
                    .unwrap()
                )
            );
        }
    }
    drop(guard);
}
#[test]
fn command_claims_reject_path_components_before_accessing_records() {
    let guard = locked_with_env(&[]);
    for command_id in ["../outside", "a/b", "a\\b", "", "a\nb"] {
        let root = prepare_root("invalid-claim");
        fs::write(root.join("state/dispatch"), command_id).unwrap();
        let output = claim(&root, "python", &guard);
        assert!(!output.status.success());
        assert!(
            String::from_utf8(output.stderr)
                .unwrap()
                .contains("command id is not a safe path component")
        );
        assert!(!root.join("outside").exists());
        assert!(!root.join("commands").exists());
    }
    drop(guard);
}
#[test]
#[cfg(windows)]
fn preparation_failure_publishes_completion_and_release() {
    let _guard = locked_with_env(&[]);
    let root = prepare_root("failed-preparation");
    let directory = prepare_record(&root, root.to_str().unwrap());
    fs::remove_file(directory.join("input/command.txt")).unwrap();
    let mut command =
        portable_pty::CommandBuilder::new(PathBuf::from(env!("CARGO_BIN_EXE_functerm")));
    command.args([
        "internal-claim-command",
        "--session-root",
        root.to_str().unwrap(),
        "--shell",
        "python",
    ]);
    command.env("FUNCTERM_SHIM_DIR", root.join("shims"));
    let (status, captured) = terminal_fixture::run_with_status(command);
    assert!(!status.success(), "{captured}");
    let done = fs::read_to_string(directory.join("state/done.json")).unwrap();
    let result: Value = sonic_rs::from_str(&done).unwrap();
    assert_eq!(result.get("exit_code").unwrap().as_i64(), Some(1_i64));
    assert_eq!(
        result.get("time_consumption").unwrap().as_str(),
        Some("0ns")
    );
    assert!(directory.join("state/released").exists(), "{captured}");
    let stderr = fs::read_to_string(directory.join("output/stderr.txt")).unwrap();
    assert!(stderr.contains("command input is missing"), "{stderr}");
}
#[test]
fn posix_management_failure_finishes_without_stranding_the_shell() {
    #[cfg(windows)]
    let shells = ["bash"];
    #[cfg(not(windows))]
    let shells = ["bash", "zsh"];
    for shell in shells {
        let _guard = locked_with_env(&[]);
        let root = temp_dir("posix-finalization");
        let tab = crate::support::create_tab(&root, shell);
        let output = crate::support::send_command(
            &tab.tab_id,
            "functerm_ensure_shim_path() { functerm_ensure_shim_path() { :; }; return 1; }; printf 'USER_COMMAND_EXECUTED\\n'",
            5.0,
        );
        let result = crate::support::parse_command_result(&output);
        assert!(result.finished, "{output:?}");
        assert_eq!(result.exit_code, Some(1_i32));
        assert!(result.stdout.contains("USER_COMMAND_EXECUTED"));
        assert!(result.stderr.contains("failed to restore shell shim path"));
        let next = crate::support::parse_command_result(&crate::support::send_command(
            &tab.tab_id,
            "printf 'SHELL_READY_AGAIN\\n'",
            5.0,
        ));
        assert_eq!(next.exit_code, Some(0_i32));
        assert!(next.stdout.contains("SHELL_READY_AGAIN"));
    }
}
fn prepare_root(leaf: &str) -> PathBuf {
    let root = temp_dir(leaf).join("space ' & % root");
    fs::create_dir_all(root.join("state")).unwrap();
    root
}
fn prepare_record(root: &Path, cwd: &str) -> PathBuf {
    let directory = root.join("commands").join("receipt-command");
    for child in ["input", "output", "state"] {
        fs::create_dir_all(directory.join(child)).unwrap();
    }
    for name in ["command.txt", "command.ps1", "command.cmd"] {
        fs::write(directory.join("input").join(name), "echo receipt").unwrap();
    }
    fs::write(directory.join("input/cwd.txt"), cwd).unwrap();
    fs::write(root.join("state/dispatch"), "receipt-command").unwrap();
    directory
}
fn claim(root: &Path, shell: &str, guard: &crate::support::TestGuard) -> Output {
    let mut environment = guard.env();
    environment.push((
        "FUNCTERM_SHIM_DIR".to_owned(),
        root.join("shims").to_str().unwrap().to_owned(),
    ));
    run_cli_with_env(
        &[
            "internal-claim-command",
            "--session-root",
            root.to_str().unwrap(),
            "--shell",
            shell,
        ],
        &environment,
    )
}
