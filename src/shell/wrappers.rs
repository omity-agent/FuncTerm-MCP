mod posix;
mod runners;
mod start;
mod template;
mod variables;
pub(super) use posix::{bash_wrapper, zsh_wrapper};
pub(super) use runners::{cmd_wrapper, nushell_wrapper, powershell_wrapper};
pub(super) use template::cmd_dispatcher;
pub(in crate::shell) use variables::{
    VariableNamespace, quoted_protected_environment_names, startup_environment,
};
