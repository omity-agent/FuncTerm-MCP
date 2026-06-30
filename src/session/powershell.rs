use std::path::Path;
pub(super) fn power_shell_args(cwd: &Path) -> Vec<String> {
    let init = format!(
        "{}\nSet-Location -LiteralPath {}",
        include_str!("../powershell_init.ps1"),
        ps_quote(cwd)
    );
    vec![
        "-NoLogo".to_owned(),
        "-NoProfile".to_owned(),
        "-NoExit".to_owned(),
        "-ExecutionPolicy".to_owned(),
        "Bypass".to_owned(),
        "-Command".to_owned(),
        init,
    ]
}
pub(super) fn ps_quote(path: &Path) -> String {
    let text = path.to_string_lossy().replace('\'', "''");
    format!("'{text}'")
}
#[cfg(test)]
#[expect(
    clippy::inline_modules,
    reason = "Rust skill permits inline modules guarded by cfg(test)"
)]
mod tests {
    use std::path::Path;
    #[test]
    fn quotes_literal_paths_for_powershell() {
        let quoted = super::ps_quote(Path::new("F:\\dir with ' quote"));
        assert_eq!(quoted, "'F:\\dir with '' quote'");
    }
}
