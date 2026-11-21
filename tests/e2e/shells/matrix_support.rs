use functerm::shell::quote;
use std::fs;
use std::path::{Path, PathBuf};
pub(super) struct ShellCase {
    pub(super) name: &'static str,
    pub(super) expected_exit_code: i32,
}
pub(super) const fn shell_cases() -> &'static [ShellCase] {
    &[
        #[cfg(windows)]
        ShellCase {
            name: "powershell",
            expected_exit_code: 7,
        },
        ShellCase {
            name: "bash",
            expected_exit_code: 1,
        },
        #[cfg(windows)]
        ShellCase {
            name: "nu",
            expected_exit_code: 0,
        },
        #[cfg(windows)]
        ShellCase {
            name: "cmd",
            expected_exit_code: 7,
        },
        #[cfg(not(windows))]
        ShellCase {
            name: "zsh",
            expected_exit_code: 1,
        },
    ]
}
pub(super) fn daemon() -> crate::support::TestGuard {
    crate::support::locked_with_env(&[])
}
pub(super) fn case_dir(shell: &str, leaf: &str) -> PathBuf {
    let path = crate::support::temp_dir(shell)
        .join("quote ' segment")
        .join(leaf);
    fs::create_dir_all(&path).unwrap();
    path
}
pub(super) fn case_command(shell: &str, next: &Path) -> String {
    match shell {
        "powershell" => format!(
            "Write-Output 'MCP_PTY_STDOUT'; Write-Error 'MCP_PTY_STDERR'; Set-Location -LiteralPath {}; cmd /c exit 7",
            quote::powershell_path(next).unwrap()
        ),
        "bash" | "zsh" => format!(
            "printf 'MCP_PTY_STDOUT\\n'; printf 'MCP_PTY_STDERR\\n' >&2; cd {}; false",
            quote::posix_string(&quote::native_path(next).unwrap().replace('\\', "/"))
        ),
        "nu" => format!(
            "print 'MCP_PTY_STDOUT'; print --stderr 'MCP_PTY_STDERR'; cd {}",
            quote::nushell_path(next).unwrap()
        ),
        "cmd" => format!(
            "echo MCP_PTY_STDOUT& echo MCP_PTY_STDERR 1>&2& cd /d {}& exit /b 7",
            quote::cmd_string(&quote::native_path(next).unwrap())
        ),
        other => panic!("unsupported shell case {other}"),
    }
}
pub(super) fn nested_launch_command(shell: &str) -> &'static str {
    match shell {
        "powershell" => "pwsh -NoLogo",
        "bash" => "bash -i",
        "nu" => "nu --no-history",
        "zsh" => "zsh -i",
        "cmd" => "cmd",
        other => panic!("unsupported shell case {other}"),
    }
}
pub(super) fn nested_marker_command(shell: &str, marker: &str) -> String {
    match shell {
        "powershell" => format!("Write-Output {}", quote::powershell_string(marker)),
        "bash" | "zsh" => format!("printf '%s\\n' {}", quote::posix_string(marker)),
        "nu" => format!("print {}", quote::nushell_string(marker)),
        "cmd" => format!("echo {marker}"),
        other => panic!("unsupported shell case {other}"),
    }
}
pub(super) fn exit_command(shell: &str) -> &'static str {
    match shell {
        "powershell" | "bash" | "nu" | "zsh" | "cmd" => "exit 42",
        other => panic!("unsupported shell case {other}"),
    }
}
pub(super) fn assert_shell_query(query: &crate::support::TabView, cwd: &Path, shell: &str) {
    assert!(query.alive, "{shell} view should report live shell");
    assert_cwd(&query.cwd, cwd, shell);
    assert!(
        !query.screen.is_empty(),
        "{shell} screen should be reported"
    );
}
pub(super) fn assert_cwd(actual: &str, expected: &Path, shell: &str) {
    let leaf = expected.file_name().unwrap().to_string_lossy();
    assert!(
        actual.replace('\\', "/").contains(&leaf.replace('\\', "/")),
        "{shell} cwd should include {}, got {actual}",
        expected.display()
    );
}
