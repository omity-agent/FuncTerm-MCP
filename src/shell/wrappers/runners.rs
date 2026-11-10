mod batch;
mod nu;
mod pwsh;
pub(in crate::shell) use batch::wrapper as cmd_wrapper;
pub(in crate::shell) use nu::wrapper as nushell_wrapper;
pub(in crate::shell) use pwsh::wrapper as powershell_wrapper;
