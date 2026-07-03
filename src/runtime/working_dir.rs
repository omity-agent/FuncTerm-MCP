use anyhow::{Context as _, Result};
use std::path::{Path, PathBuf};
pub(crate) fn resolve(input: Option<&Path>) -> Result<PathBuf> {
    let base = std::env::current_dir().context("failed to locate program working directory")?;
    let Some(raw_path) = input else {
        return Ok(base);
    };
    if raw_path.is_absolute() {
        return Ok(raw_path.to_path_buf());
    }
    Ok(base.join(raw_path))
}
#[cfg(test)]
mod tests {
    use std::path::Path;
    #[test]
    fn keeps_current_directory_when_input_is_absent() {
        let resolved = super::resolve(None).unwrap();
        assert_eq!(resolved, std::env::current_dir().unwrap());
    }
    #[test]
    fn resolves_relative_path_against_process_current_directory() {
        let resolved = super::resolve(Some(Path::new("child"))).unwrap();
        assert_eq!(resolved, std::env::current_dir().unwrap().join("child"));
    }
    #[test]
    fn keeps_shell_syntax_literals_unexpanded() {
        let resolved = super::resolve(Some(Path::new("$FUNCTERM_ROOT"))).unwrap();
        assert_eq!(
            resolved,
            std::env::current_dir().unwrap().join("$FUNCTERM_ROOT")
        );
    }
}
