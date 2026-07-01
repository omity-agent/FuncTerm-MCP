use iceoryx2::prelude::ZeroCopySend;
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub(crate) struct RequestHeader {
    pub(crate) kind: u8,
    pub(crate) shell: u8,
    pub(crate) reserved: [u8; 6],
    pub(crate) wait_ms: u64,
    pub(crate) cwd_len: u64,
    pub(crate) shell_id_len: u64,
    pub(crate) command_len: u64,
    pub(crate) keyboard_len: u64,
    pub(crate) query_id_len: u64,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[repr(C)]
pub(crate) struct ResponseHeader {
    pub(crate) status: u8,
    pub(crate) payload_kind: u8,
    pub(crate) end_reason: u8,
    pub(crate) query_kind: u8,
    pub(crate) alive: u8,
    pub(crate) finished: u8,
    pub(crate) has_exit_code: u8,
    pub(crate) reserved: u8,
    pub(crate) exit_code: i32,
    pub(crate) message_len: u64,
    pub(crate) shell_id_len: u64,
    pub(crate) command_id_len: u64,
    pub(crate) cwd_len: u64,
    pub(crate) screen_len: u64,
    pub(crate) stdout_len: u64,
    pub(crate) stderr_len: u64,
}
unsafe impl ZeroCopySend for RequestHeader {
    unsafe fn type_name() -> &'static str {
        "shell_mcp_pty.protocol.RequestHeader.v1"
    }
    fn __is_zero_copy_send(&self) {}
}
unsafe impl ZeroCopySend for ResponseHeader {
    unsafe fn type_name() -> &'static str {
        "shell_mcp_pty.protocol.ResponseHeader.v1"
    }
    fn __is_zero_copy_send(&self) {}
}
