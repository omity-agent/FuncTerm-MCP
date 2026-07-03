use core::sync::atomic::{AtomicU64, Ordering};
use functerm::shell::quote;
use std::fs;
use std::path::{Path, PathBuf};
static CASE_DIR_COUNTER: AtomicU64 = AtomicU64::new(0);
pub(super) struct ShellCase {
    pub(super) name: &'static str,
    pub(super) env_var: &'static str,
    pub(super) executables: &'static [&'static str],
    pub(super) expected_exit_code: i32,
}
#[cfg(windows)]
pub(super) fn shell_cases() -> &'static [ShellCase] {
    &[
        ShellCase {
            name: "powershell",
            env_var: "SHELL_MCP_PTY_POWERSHELL",
            executables: &["pwsh", "pwsh.exe", "powershell", "powershell.exe"],
            expected_exit_code: 7,
        },
        ShellCase {
            name: "bash",
            env_var: "SHELL_MCP_PTY_BASH",
            executables: &["bash", "bash.exe"],
            expected_exit_code: 1,
        },
        ShellCase {
            name: "nu",
            env_var: "SHELL_MCP_PTY_NUSHELL",
            executables: &["nu", "nu.exe"],
            expected_exit_code: 0,
        },
    ]
}
#[cfg(not(windows))]
pub(super) fn shell_cases() -> &'static [ShellCase] {
    &[
        ShellCase {
            name: "bash",
            env_var: "SHELL_MCP_PTY_BASH",
            executables: &["bash", "bash.exe"],
            expected_exit_code: 1,
        },
        ShellCase {
            name: "zsh",
            env_var: "SHELL_MCP_PTY_ZSH",
            executables: &["zsh"],
            expected_exit_code: 1,
        },
    ]
}
pub(super) fn required_executable(case: &ShellCase) -> String {
    case.executables
        .iter()
        .find_map(|executable| which::which(executable).ok())
        .map_or_else(
            || {
                panic!(
                    "CI must install {name}; tried {executables}",
                    name = case.name,
                    executables = case.executables.join(", ")
                )
            },
            |path| path.to_string_lossy().into_owned(),
        )
}
#[cfg(windows)]
pub(super) fn immediately_exiting_executable() -> &'static str {
    "where.exe"
}
#[cfg(not(windows))]
pub(super) fn immediately_exiting_executable() -> &'static str {
    "false"
}
pub(super) fn case_dir(shell: &str, leaf: &str) -> PathBuf {
    let unique = CASE_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir()
        .join("functerm-cli")
        .join(format!("{shell}-{}-{unique}", std::process::id()))
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
        other => panic!("unsupported shell case {other}"),
    }
}
pub(super) fn nested_launch_command(shell: &str) -> &'static str {
    match shell {
        "powershell" => "pwsh",
        "bash" => "bash",
        "nu" => "nu",
        "zsh" => "zsh",
        other => panic!("unsupported shell case {other}"),
    }
}
pub(super) fn nested_marker_command(shell: &str, marker: &str) -> String {
    match shell {
        "powershell" => format!("Write-Output {}", quote::powershell_string(marker)),
        "bash" | "zsh" => format!("printf '%s\\n' {}", quote::posix_string(marker)),
        "nu" => format!("print {}", quote::nushell_string(marker)),
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
