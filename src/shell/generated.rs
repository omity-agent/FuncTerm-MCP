mod nu;
mod posix;
mod posix_dialect;
mod pwsh;
pub(super) use nu::wrapper as nushell_wrapper;
pub(super) use posix::{bash_wrapper, zsh_wrapper};
pub(super) use pwsh::wrapper as powershell_wrapper;
