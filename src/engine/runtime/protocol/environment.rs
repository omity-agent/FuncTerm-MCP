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
mod tests {
    use super::EnvironmentSnapshot;
    use std::ffi::OsString;
    #[test]
    fn environment_variables_round_trip() {
        let value = OsString::from("environment value");
        let snapshot = EnvironmentSnapshot::from_variables([(
            OsString::from("FUNCTERM_TEST_NAME"),
            value.clone(),
        )]);
        assert_eq!(snapshot.value("FUNCTERM_TEST_NAME"), Some(value));
    }
    #[test]
    fn snapshot_round_trips_through_ipc_json() {
        let snapshot = EnvironmentSnapshot::from_variables([(
            OsString::from("FUNCTERM_TEST_NAME"),
            OsString::from("snapshot value"),
        )]);
        let json = sonic_rs::to_string(&snapshot).unwrap();
        let decoded = sonic_rs::from_str::<EnvironmentSnapshot>(&json).unwrap();
        assert_eq!(
            decoded.value("FUNCTERM_TEST_NAME"),
            Some(OsString::from("snapshot value"))
        );
    }
    #[cfg(windows)]
    #[test]
    fn new_tab_request_does_not_send_client_environment() {
        assert_eq!(
            EnvironmentSnapshot::for_new_tab_request().variables(),
            Vec::new()
        );
    }
}
