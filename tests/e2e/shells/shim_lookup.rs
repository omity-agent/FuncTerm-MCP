use super::matrix::{case_dir, shell_cases};
use crate::support::{complete_command, create_tab};
#[test]
fn cli_is_available_with_only_shim_path() {
    let _guard = super::matrix::daemon();
    for shell in shell_cases()
        .iter()
        .map(|case| case.name)
        .chain(["python", "bun"])
    {
        assert_cli_lookup(shell);
    }
}
#[cfg(windows)]
#[test]
fn powershell_lookup_ranks_all_path_hits_and_packaged_previews() {
    let guard = crate::support::locked_with_env(&[]);
    let local = crate::support::temp_dir("packaged-shell");
    let first = local.join(r"first\pwsh.exe");
    let later = local.join(r"later\pwsh.exe");
    let packaged = local
        .join(r"Microsoft\WindowsApps\Microsoft.PowerShellPreview_8wekyb3d8bbwe\pwsh-preview.exe");
    let windows = std::path::PathBuf::from(std::env::var_os("SystemRoot").unwrap());
    let compiler = windows.join(r"Microsoft.NET\Framework\v4.0.30319\csc.exe");
    for (executable, version) in [
        (&first, "99.9.0"),
        (&later, "99.10.0-preview.2"),
        (&packaged, "99.10.0-preview.10"),
    ] {
        std::fs::create_dir_all(executable.parent().unwrap()).unwrap();
        let source = executable.with_extension("cs");
        std::fs::write(
            &source,
            format!(
                r#"class Probe {{ static void Main() {{ System.Console.WriteLine("PowerShell {version}"); }} }}"#
            ),
        )
        .unwrap();
        let output = std::process::Command::new(&compiler)
            .arg(format!("/out:{}", executable.display()))
            .arg(&source)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }
    let system = windows.join("System32");
    let path =
        std::env::join_paths([first.parent().unwrap(), later.parent().unwrap(), &system]).unwrap();
    let mut environment = guard.env();
    environment.extend([
        (
            "LOCALAPPDATA".to_owned(),
            local.to_str().unwrap().to_owned(),
        ),
        ("PATH".to_owned(), path.into_string().unwrap()),
    ]);
    for expected in [packaged, later] {
        let tab = crate::support::create_tab_with_env(&local, "cmd", &environment);
        let resolved = complete_command(&tab.tab_id, "set FUNCTERM_REAL_POWERSHELL");
        assert_eq!(resolved.exit_code, Some(0_i32), "{resolved:?}");
        assert_eq!(
            resolved.stdout.trim(),
            format!("FUNCTERM_REAL_POWERSHELL={}", expected.display())
        );
        std::fs::remove_file(expected).unwrap();
    }
}
fn assert_cli_lookup(shell: &str) {
    let directory = case_dir(shell, "shim lookup");
    let created = create_tab(&directory, shell);
    let initial = complete_command(&created.tab_id, &executable_probe_command(shell, false));
    assert_eq!(initial.exit_code, Some(0_i32), "{shell}: {initial:?}");
    assert!(initial.stdout.contains("Usage:"), "{shell}: {initial:?}");
    let next = complete_command(&created.tab_id, &executable_probe_command(shell, true));
    assert_eq!(next.exit_code, Some(0_i32), "{shell}: {next:?}");
    assert!(next.stdout.contains("Usage:"), "{next:?}");
    assert!(!directory.join("done.json").exists());
}
fn executable_probe_command(shell: &str, isolated: bool) -> String {
    let (prefix, invocation) = match shell {
        "powershell" => ("$env:PATH = $env:FUNCTERM_SHIM_DIR; ", "functerm --help"),
        "bash" | "zsh" => ("export PATH=\"${PATH%%:*}\"; ", "functerm --help"),
        "nu" => ("$env.PATH = [$env.FUNCTERM_SHIM_DIR]; ", "functerm --help"),
        "cmd" => ("set \"PATH=%FUNCTERM_SHIM_DIR%\"& ", "functerm --help"),
        "python" => (
            "__import__('os').environ['PATH'] = __import__('os').environ['FUNCTERM_SHIM_DIR']; ",
            "__import__('subprocess').run(['functerm', '--help'], check=True)",
        ),
        "bun" => (
            "process.env.PATH = process.env.FUNCTERM_SHIM_DIR; ",
            "(await import('node:child_process')).execFileSync('functerm', ['--help'], {stdio: 'inherit'})",
        ),
        other => panic!("unsupported shell case {other}"),
    };
    format!("{}{invocation}", if isolated { prefix } else { "" })
}
