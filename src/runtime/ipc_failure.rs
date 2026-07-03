use ipc_channel::{IpcError, TryRecvError};
pub(super) fn is_retryable_bootstrap_error(error: &anyhow::Error) -> bool {
    has_busy_bootstrap_pipe(error) || has_disconnected_bootstrap_reply(error)
}
fn has_busy_bootstrap_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|source| {
        source
            .downcast_ref::<std::io::Error>()
            .is_some_and(is_busy_bootstrap_pipe)
    })
}
fn is_busy_bootstrap_pipe(error: &std::io::Error) -> bool {
    is_platform_busy_bootstrap_pipe(error)
}
#[cfg(windows)]
fn is_platform_busy_bootstrap_pipe(error: &std::io::Error) -> bool {
    use windows_sys::Win32::Foundation::{ERROR_PIPE_BUSY, ERROR_PIPE_LISTENING};
    const HRESULT_ERROR_PIPE_BUSY: i32 = -2_147_024_665;
    const HRESULT_ERROR_PIPE_LISTENING: i32 = -2_147_024_360;
    let Some(raw_error) = error.raw_os_error() else {
        return false;
    };
    raw_error == HRESULT_ERROR_PIPE_BUSY
        || raw_error == HRESULT_ERROR_PIPE_LISTENING
        || u32::try_from(raw_error)
            .is_ok_and(|code| code == ERROR_PIPE_BUSY || code == ERROR_PIPE_LISTENING)
}
#[cfg(not(windows))]
fn is_platform_busy_bootstrap_pipe(_error: &std::io::Error) -> bool {
    false
}
fn has_disconnected_bootstrap_reply(error: &anyhow::Error) -> bool {
    error.chain().any(|source| {
        matches!(
            source.downcast_ref::<TryRecvError>(),
            Some(TryRecvError::IpcError(IpcError::Disconnected))
        )
    })
}
