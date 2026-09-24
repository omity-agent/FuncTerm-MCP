use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
#[cfg(windows)]
mod windows;
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct EnvironmentSnapshot {
    variables: Vec<(OsString, OsString)>,
}
impl EnvironmentSnapshot {
    #[cfg(any(not(windows), test))]
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
    #[cfg(windows)]
    pub(crate) fn for_new_tab_request() -> Self {
        Self::from_variables([])
    }
    #[cfg(not(windows))]
    pub(crate) fn for_new_tab_request() -> Self {
        Self::capture()
    }
    #[cfg(windows)]
    pub(crate) fn tab_launch_environment(_client_environment: &Self) -> anyhow::Result<Self> {
        windows::capture_user_environment()
    }
    #[cfg(not(windows))]
    #[expect(
        clippy::unnecessary_wraps,
        reason = "the same launch interface returns errors when Windows environment capture fails"
    )]
    pub(crate) fn tab_launch_environment(client_environment: &Self) -> anyhow::Result<Self> {
        Ok(client_environment.clone())
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
#[cfg(test)]
#[path = "../../../../tests/unit/platform/snapshot.rs"]
mod tests;
