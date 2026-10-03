#[cfg(test)]
#[cfg(windows)]
#[path = "integration/bootstrap_contract.rs"]
mod bootstrap_contract;
#[cfg(test)]
#[path = "e2e/streams.rs"]
mod routing;
#[cfg(test)]
#[path = "e2e/services.rs"]
mod services;
#[cfg(test)]
#[path = "integration/session_policy.rs"]
mod session_policy;
#[cfg(test)]
#[path = "e2e/shells.rs"]
mod shells;
#[cfg(test)]
#[cfg(unix)]
#[path = "integration/startup_files.rs"]
mod startup_files;
#[path = "e2e/harness.rs"]
mod support;
