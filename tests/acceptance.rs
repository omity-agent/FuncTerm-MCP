#![cfg_attr(windows, feature(windows_process_extensions_raw_attribute))]
#[path = "integration/administration.rs"]
mod administration;
#[path = "integration/semantics/fidelity.rs"]
mod fidelity;
#[cfg(windows)]
#[path = "integration/identity_boundary.rs"]
mod identity_boundary;
#[path = "integration/semantics/interactive_state.rs"]
mod interactive_state;
#[cfg(windows)]
#[path = "integration/job_isolation.rs"]
mod job_isolation;
#[path = "integration/permission_policy.rs"]
mod permission_policy;
#[path = "e2e/streams.rs"]
mod routing;
#[path = "e2e/services.rs"]
mod services;
#[path = "e2e/shells.rs"]
mod shells;
#[cfg(unix)]
#[path = "integration/startup_files.rs"]
mod startup_files;
#[path = "integration/semantics/submission.rs"]
mod submission;
#[path = "e2e/harness.rs"]
mod support;
