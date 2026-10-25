#![cfg_attr(windows, feature(windows_process_extensions_raw_attribute))]
#[cfg(test)]
#[path = "integration/administration.rs"]
mod administration;
#[cfg(test)]
#[cfg(windows)]
#[path = "integration/job_isolation.rs"]
mod job_isolation;
#[cfg(test)]
#[path = "integration/permission_policy.rs"]
mod permission_policy;
#[cfg(test)]
#[path = "e2e/streams.rs"]
mod routing;
#[cfg(test)]
#[path = "e2e/services.rs"]
mod services;
#[cfg(test)]
#[path = "e2e/shells.rs"]
mod shells;
#[cfg(test)]
#[cfg(unix)]
#[path = "integration/startup_files.rs"]
mod startup_files;
#[path = "e2e/harness.rs"]
mod support;
