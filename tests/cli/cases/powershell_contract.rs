#[cfg(test)]
mod tests {
    use crate::support::{
        create_tab, locked, parse_command_result, parse_tab_view, required_executable, run_cli,
        send_command, temp_root,
    };
    use std::path::{Path, PathBuf};
    use std::process::Command;
    const CHECK_SCRIPT: &str = "
$ErrorActionPreference = 'Stop'
$tokens = $null
$parseErrors = $null
$null = [System.Management.Automation.Language.Parser]::ParseFile(
    $env:FUNCTERM_TEST_POWERSHELL_SCRIPT,
    [ref] $tokens,
    [ref] $parseErrors
)
if ($parseErrors.Count -gt 0) {
    $messages = $parseErrors | ForEach-Object {
        '{0}:{1}: {2}' -f $_.Extent.StartLineNumber, $_.Extent.StartColumnNumber, $_.Message
    }
    [Console]::Error.WriteLine($messages -join [Environment]::NewLine)
    exit 2
}
Import-Module -Name PSScriptAnalyzer -ErrorAction Stop
$scriptDefinition = [IO.File]::ReadAllText($env:FUNCTERM_TEST_POWERSHELL_SCRIPT)
$formatted = Invoke-Formatter -ScriptDefinition $scriptDefinition
[IO.File]::WriteAllText(
    $env:FUNCTERM_TEST_FORMATTED_SCRIPT,
    $formatted,
    [Text.UTF8Encoding]::new($false)
)
";
    #[test]
    fn rendered_powershell_initialization_passes_parser_and_formatter() {
        let _guard = locked();
        let executable = required_executable(&["pwsh", "pwsh.exe", "powershell", "powershell.exe"]);
        let tab = create_tab(&temp_root(), "powershell");
        let rendered_path = rendered_script_path(&tab.tab_id);
        let formatted_path = rendered_path.with_file_name("powershell_init.formatted.ps1");
        let output = Command::new(executable)
            .args([
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                CHECK_SCRIPT,
            ])
            .env("FUNCTERM_TEST_POWERSHELL_SCRIPT", &rendered_path)
            .env("FUNCTERM_TEST_FORMATTED_SCRIPT", &formatted_path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "PowerShell parser or formatter rejected {}:\nstdout: {}\nstderr: {}",
            rendered_path.display(),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let rendered = std::fs::read_to_string(&rendered_path).unwrap();
        let formatted = std::fs::read_to_string(&formatted_path).unwrap();
        assert_eq!(
            rendered, formatted,
            "Invoke-Formatter changed the rendered PowerShell initialization script"
        );
    }
    #[test]
    fn powershell_user_errors_hide_wrapper_details_and_keep_tab_usable() {
        let _guard = locked();
        let tab = create_tab(&temp_root(), "powershell");
        for (command, diagnostic) in [
            (
                r#"rg --line-number "FileLinkMenuOpen|useSourceHover|HighlightedCode|useSyncExternalStore|addEventListener\(\"(keydown|keyup)" src/app/frontend tests/app/frontend"#,
                "(keydown|keyup)",
            ),
            (
                "Write-Output 'FUNCTERM_SHOULD_NOT_EXECUTE'\nWrite-Output \"unterminated",
                "unterminated",
            ),
            (
                "throw 'FUNCTERM_USER_RUNTIME_ERROR'",
                "FUNCTERM_USER_RUNTIME_ERROR",
            ),
        ] {
            let output = send_command(&tab.tab_id, command, 5.0);
            let result = parse_command_result(&output);
            assert!(result.finished, "command should finish: {}", result.stderr);
            assert_eq!(result.exit_code, Some(1_i32), "{}", result.stderr);
            assert!(result.stdout.trim().is_empty(), "{}", result.stdout);
            assert!(result.stderr.contains(diagnostic), "{}", result.stderr);
            let view = parse_tab_view(&run_cli(&["view", &tab.tab_id]));
            assert!(view.alive, "user errors should not close the tab");
            for text in [&result.stderr, &view.screen] {
                for internal in [
                    "powershell_init.ps1",
                    "commandScript_",
                    "[scriptblock]::Create",
                    "MethodInvocationException",
                ] {
                    assert!(
                        !text.contains(internal),
                        "user error exposed {internal}:\n{text}"
                    );
                }
            }
        }
        let recovered = parse_command_result(&send_command(
            &tab.tab_id,
            "Write-Output 'FUNCTERM_AFTER_USER_ERROR'",
            5.0,
        ));
        assert!(recovered.finished);
        assert_eq!(recovered.exit_code, Some(0_i32), "{}", recovered.stderr);
        assert!(recovered.stdout.contains("FUNCTERM_AFTER_USER_ERROR"));
        assert!(recovered.stderr.is_empty(), "{}", recovered.stderr);
    }
    fn rendered_script_path(tab_id: &str) -> PathBuf {
        let services = temp_root().join("functerm").join("services");
        let mut matches = child_directories(&services)
            .into_iter()
            .flat_map(|service| child_directories(&service.join("generations")))
            .map(|generation| {
                generation
                    .join("tabs")
                    .join(tab_id)
                    .join("startup")
                    .join("powershell_init.ps1")
            })
            .filter(|path| path.is_file())
            .collect::<Vec<_>>();
        assert_eq!(
            matches.len(),
            1,
            "expected one rendered PowerShell script for {tab_id}, found {matches:?}"
        );
        matches.pop().unwrap()
    }
    fn child_directories(path: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()))
            .map(|entry| entry.unwrap().path())
            .filter(|child| child.is_dir())
            .collect()
    }
}
