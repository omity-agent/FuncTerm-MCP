use serde::{Deserialize, Serialize};
use std::ffi::{OsStr, OsString};
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct EnvironmentSnapshot {
    variables: Vec<(NativeString, NativeString)>,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
struct NativeString {
    #[cfg(unix)]
    units: Vec<u8>,
    #[cfg(windows)]
    units: Vec<u16>,
}
impl EnvironmentSnapshot {
    pub(crate) fn capture() -> Self {
        Self::from_variables(std::env::vars_os())
    }
    pub(crate) fn from_variables(
        variables: impl IntoIterator<Item = (OsString, OsString)>,
    ) -> Self {
        Self {
            variables: variables
                .into_iter()
                .map(|(name, value)| (NativeString::from(name), NativeString::from(value)))
                .collect(),
        }
    }
    pub(crate) fn variables(&self) -> Vec<(OsString, OsString)> {
        self.variables
            .iter()
            .map(|pair| (pair.0.to_os_string(), pair.1.to_os_string()))
            .collect()
    }
    pub(crate) fn value(&self, expected_name: &str) -> Option<OsString> {
        for pair in &self.variables {
            let decoded_name = pair.0.to_os_string();
            if environment_name_equals(&decoded_name, expected_name) {
                return Some(pair.1.to_os_string());
            }
        }
        None
    }
}
impl From<OsString> for NativeString {
    fn from(value: OsString) -> Self {
        Self::from_os_str(&value)
    }
}
impl NativeString {
    #[cfg(unix)]
    fn from_os_str(value: &OsStr) -> Self {
        use std::os::unix::ffi::OsStrExt as _;
        Self {
            units: value.as_bytes().to_vec(),
        }
    }
    #[cfg(unix)]
    fn to_os_string(&self) -> OsString {
        use std::os::unix::ffi::OsStringExt as _;
        OsString::from_vec(self.units.clone())
    }
    #[cfg(windows)]
    fn from_os_str(value: &OsStr) -> Self {
        use std::os::windows::ffi::OsStrExt as _;
        Self {
            units: value.encode_wide().collect(),
        }
    }
    #[cfg(windows)]
    fn to_os_string(&self) -> OsString {
        use std::os::windows::ffi::OsStringExt as _;
        OsString::from_wide(&self.units)
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
    use super::{EnvironmentSnapshot, NativeString};
    use std::ffi::OsString;
    #[test]
    fn native_string_round_trips() {
        let value = OsString::from("environment value");
        assert_eq!(NativeString::from(value.clone()).to_os_string(), value);
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
}
