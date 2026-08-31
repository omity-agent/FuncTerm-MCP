#[cfg(test)]
mod tests {
    use crate::support::{create_tab, locked, required_executable, temp_root};
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
