use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct EnvironmentSnapshot {
    variables: Vec<(OsString, OsString)>,
}
impl EnvironmentSnapshot {
    pub(crate) fn capture() -> Self {
        Self::from_variables(std::env::vars_os())
    }
    pub(crate) fn from_variables(
        variables: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Self {
        Self {
            variables: variables.into_iter().collect(),
        }
    }
    pub(crate) fn variables(&self) -> Vec<(OsString, OsString)> {
        self.variables.clone()
    }
    pub(crate) fn value(&self, expected_name: &str) -> Option<OsString> {
        self.variables.iter().find_map(|pair| {
            environment_name_equals(pair.0.as_os_str(), expected_name).then(|| pair.1.clone())
        })
    }
}
#[cfg(windows)]
pub(crate) fn environment_name_equals(actual: &OsStr, expected: &str) -> bool {
    actual.eq_ignore_ascii_case(expected)
}
#[cfg(not(windows))]
pub(crate) fn environment_name_equals(actual: &OsStr, expected: &str) -> bool {
    actual == expected
}
