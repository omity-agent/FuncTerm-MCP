use anyhow::{Context as _, Result, bail};
use core::iter::Peekable;
use core::str::Chars;
use std::path::{Path, PathBuf};
pub(crate) fn resolve(input: Option<&Path>) -> Result<PathBuf> {
    let base = std::env::current_dir().context("failed to locate program working directory")?;
    let Some(raw_path) = input else {
        return Ok(base);
    };
    let expanded_path = expand_path(raw_path)?;
    if expanded_path.is_absolute() {
        return Ok(expanded_path);
    }
    Ok(base.join(expanded_path))
}
fn expand_path(path: &Path) -> Result<PathBuf> {
    let path_text = path
        .to_str()
        .with_context(|| format!("starting_directory is not valid UTF-8: {}", path.display()))?;
    let expanded_text = expand_environment_variables(path_text, environment_value)?;
    Ok(PathBuf::from(expanded_text))
}
fn environment_value(name: &str) -> Result<String> {
    match std::env::var(name) {
        Ok(value) => Ok(value),
        Err(std::env::VarError::NotPresent) => bail!("environment variable {name} is not set"),
        Err(std::env::VarError::NotUnicode(_)) => {
            bail!("environment variable {name} is not valid Unicode")
        }
    }
}
fn expand_environment_variables(
    input: &str,
    mut variable_value: impl FnMut(&str) -> Result<String>,
) -> Result<String> {
    let mut expanded = String::with_capacity(input.len());
    let mut characters = input.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '%' => expand_percent_variable(&mut characters, &mut expanded, &mut variable_value)?,
            '$' => {
                expand_dollar_variable(input, &mut characters, &mut expanded, &mut variable_value)?;
            }
            _ => expanded.push(character),
        }
    }
    Ok(expanded)
}
fn expand_percent_variable(
    characters: &mut Peekable<Chars<'_>>,
    expanded: &mut String,
    variable_value: &mut impl FnMut(&str) -> Result<String>,
) -> Result<()> {
    let (name, terminated) = take_until(characters, '%');
    if terminated && is_percent_name(&name) {
        expanded.push_str(&variable_value(&name)?);
        return Ok(());
    }
    expanded.push('%');
    expanded.push_str(&name);
    if terminated {
        expanded.push('%');
    }
    Ok(())
}
fn expand_dollar_variable(
    input: &str,
    characters: &mut Peekable<Chars<'_>>,
    expanded: &mut String,
    variable_value: &mut impl FnMut(&str) -> Result<String>,
) -> Result<()> {
    let Some(next_character) = characters.peek().copied() else {
        expanded.push('$');
        return Ok(());
    };
    if next_character == '{' {
        characters.next();
        return expand_braced_dollar_variable(input, characters, expanded, variable_value);
    }
    if !is_dollar_name_start(next_character) {
        expanded.push('$');
        return Ok(());
    }
    let name = take_dollar_name(characters);
    expanded.push_str(&variable_value(&name)?);
    Ok(())
}
fn expand_braced_dollar_variable(
    input: &str,
    characters: &mut Peekable<Chars<'_>>,
    expanded: &mut String,
    variable_value: &mut impl FnMut(&str) -> Result<String>,
) -> Result<()> {
    let (name, terminated) = take_until(characters, '}');
    if !terminated {
        bail!("environment variable reference is missing closing brace: {input}");
    }
    if name.is_empty() {
        bail!("environment variable reference is empty: {input}");
    }
    if !name.chars().all(is_dollar_name) {
        bail!("environment variable reference has an invalid name: {name}");
    }
    expanded.push_str(&variable_value(&name)?);
    Ok(())
}
fn take_until(characters: &mut Peekable<Chars<'_>>, terminator: char) -> (String, bool) {
    let mut text = String::new();
    for character in characters.by_ref() {
        if character == terminator {
            return (text, true);
        }
        text.push(character);
    }
    (text, false)
}
fn take_dollar_name(characters: &mut Peekable<Chars<'_>>) -> String {
    let mut name = String::new();
    while let Some(character) = characters.peek().copied() {
        if !is_dollar_name(character) {
            break;
        }
        name.push(character);
        characters.next();
    }
    name
}
fn is_percent_name(name: &str) -> bool {
    !name.is_empty() && name.chars().all(is_percent_name_character)
}
const fn is_percent_name_character(character: char) -> bool {
    character.is_ascii_alphanumeric() || matches!(character, '_' | '(' | ')' | '.' | '-')
}
const fn is_dollar_name_start(character: char) -> bool {
    character.is_ascii_alphabetic() || character == '_'
}
const fn is_dollar_name(character: char) -> bool {
    is_dollar_name_start(character) || character.is_ascii_digit()
}
#[cfg(test)]
mod tests {
    use super::expand_environment_variables;
    use anyhow::{Result, bail};
    #[test]
    fn expands_percent_variables() {
        let expanded =
            expand_environment_variables(r"%FUNCTERM_ROOT%\child\%FUNCTERM_LEAF%", test_variable)
                .unwrap();
        assert_eq!(expanded, r"F:\root\child\leaf");
    }
    #[test]
    fn expands_dollar_variables() {
        let expanded =
            expand_environment_variables("$FUNCTERM_ROOT/${FUNCTERM_LEAF}/tail", test_variable)
                .unwrap();
        assert_eq!(expanded, "F:\\root/leaf/tail");
    }
    #[test]
    fn keeps_unrecognized_dollar_literals() {
        let expanded = expand_environment_variables(r"F:\$-Recycle.Bin", test_variable).unwrap();
        assert_eq!(expanded, r"F:\$-Recycle.Bin");
    }
    #[test]
    fn rejects_missing_variables() {
        let error = expand_environment_variables("%FUNCTERM_MISSING%", test_variable).unwrap_err();
        assert_eq!(error.to_string(), "missing test variable FUNCTERM_MISSING");
    }
    #[test]
    fn rejects_invalid_braced_variable_names() {
        let error = expand_environment_variables("${FUNCTERM-ROOT}", test_variable).unwrap_err();
        assert_eq!(
            error.to_string(),
            "environment variable reference has an invalid name: FUNCTERM-ROOT"
        );
    }
    fn test_variable(name: &str) -> Result<String> {
        match name {
            "FUNCTERM_ROOT" => Ok("F:\\root".to_owned()),
            "FUNCTERM_LEAF" => Ok("leaf".to_owned()),
            other => bail!("missing test variable {other}"),
        }
    }
}
