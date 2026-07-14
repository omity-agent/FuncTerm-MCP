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
            env_var: "FUNCTERM_POWERSHELL",
            executables: &["pwsh", "pwsh.exe", "powershell", "powershell.exe"],
            expected_exit_code: 7,
        },
        ShellCase {
            name: "bash",
            env_var: "FUNCTERM_BASH",
            executables: &["bash", "bash.exe"],
            expected_exit_code: 1,
        },
        ShellCase {
            name: "nu",
            env_var: "FUNCTERM_NUSHELL",
            executables: &["nu", "nu.exe"],
            expected_exit_code: 0,
        },
        ShellCase {
            name: "cmd",
            env_var: "FUNCTERM_CMD",
            executables: &["cmd", "cmd.exe"],
            expected_exit_code: 7,
        },
    ]
}
#[cfg(not(windows))]
pub(super) fn shell_cases() -> &'static [ShellCase] {
    &[
        ShellCase {
            name: "bash",
            env_var: "FUNCTERM_BASH",
            executables: &["bash", "bash.exe"],
            expected_exit_code: 1,
        },
        ShellCase {
            name: "zsh",
            env_var: "FUNCTERM_ZSH",
            executables: &["zsh"],
            expected_exit_code: 1,
        },
    ]
}
pub(super) fn required_executable(case: &ShellCase) -> String {
    case.executables
        .iter()
        .find_map(|executable| real_executable(executable))
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
fn real_executable(executable: &str) -> Option<PathBuf> {
    which::which_all(executable)
        .ok()?
        .find(|path| !in_func_term_shim_dir(path))
}
fn in_func_term_shim_dir(path: &Path) -> bool {
    let Some(shim_dir_value) = std::env::var_os("FUNCTERM_SHIM_DIR") else {
        return false;
    };
    let Some(path_parent) = path.parent() else {
        return false;
    };
    let Ok(canonical_parent) = path_parent.canonicalize() else {
        return false;
    };
    let Ok(canonical_shim_dir) = PathBuf::from(shim_dir_value).canonicalize() else {
        return false;
    };
    canonical_parent == canonical_shim_dir
}
#[cfg(windows)]
pub(super) fn immediately_exiting_executable() -> &'static str {
    "whoami.exe"
}
#[cfg(not(windows))]
pub(super) fn immediately_exiting_executable() -> &'static str {
    "false"
}
pub(super) fn case_dir(shell: &str, leaf: &str) -> PathBuf {
    let unique = CASE_DIR_COUNTER.fetch_add(1, Ordering::Relaxed);
    let path = crate::support::temp_root()
        .join("cli")
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
        "cmd" => format!(
            "echo MCP_PTY_STDOUT& echo MCP_PTY_STDERR 1>&2& cd /d {}& exit /b 7",
            quote::cmd_string(&quote::native_path(next).unwrap())
        ),
        other => panic!("unsupported shell case {other}"),
    }
}
pub(super) fn plain_title_command(shell: &str) -> &'static str {
    match shell {
        "powershell" => "Write-Output 'MCP_PTY_PLAIN_TITLE'",
        "bash" | "zsh" => "printf 'MCP_PTY_PLAIN_TITLE\\n'",
        "nu" => "print 'MCP_PTY_PLAIN_TITLE'",
        "cmd" => "echo MCP_PTY_PLAIN_TITLE",
        other => panic!("unsupported shell case {other}"),
    }
}
pub(super) fn set_title_command(shell: &str, title: &str) -> String {
    match shell {
        "powershell" => format!(
            "[Console]::Write(\"$([char]27)]2;{title}$([char]7)\"); Write-Output 'MCP_PTY_TITLE_SET'"
        ),
        "bash" | "zsh" => format!("printf '\\033]2;{title}\\007'; printf 'MCP_PTY_TITLE_SET\\n'"),
        "nu" => format!(
            "print --raw --no-newline $'(ansi osc)2;{title}(ansi st)'; print 'MCP_PTY_TITLE_SET'"
        ),
        "cmd" => format!("title {title}& echo MCP_PTY_TITLE_SET"),
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
