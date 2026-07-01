mod codec;
mod request;
mod response;
#[cfg(test)]
mod tests;
#[cfg(test)]
pub(crate) use request::RequestFrame;
pub(crate) use request::{
    BorrowedRequest, decode_request, request_header_len, write_request_payload,
};
#[cfg(test)]
pub(crate) use response::ResponseFrame;
pub(crate) use response::{decode_response, response_header_len, write_response_payload};
pub(super) const REQUEST_PING: u8 = 0;
pub(super) const REQUEST_NEW_SHELL: u8 = 1;
pub(super) const REQUEST_WRITE_KEYBOARD: u8 = 2;
pub(super) const REQUEST_SEND_COMMAND: u8 = 3;
pub(super) const REQUEST_QUERY: u8 = 4;
pub(super) const SHELL_POWERSHELL: u8 = 0;
pub(super) const SHELL_BASH: u8 = 1;
pub(super) const SHELL_NUSHELL: u8 = 2;
pub(super) const SHELL_ZSH: u8 = 3;
pub(super) const RESPONSE_OK: u8 = 0;
pub(super) const RESPONSE_ERR: u8 = 1;
pub(super) const PAYLOAD_PONG: u8 = 0;
pub(super) const PAYLOAD_SHELL_CREATED: u8 = 1;
pub(super) const PAYLOAD_KEYBOARD_WRITTEN: u8 = 2;
pub(super) const PAYLOAD_COMMAND_ACCEPTED: u8 = 3;
pub(super) const PAYLOAD_QUERY: u8 = 4;
pub(super) const END_COMMAND_ENDED: u8 = 0;
pub(super) const END_WAIT_TIMEOUT: u8 = 1;
pub(super) const QUERY_SHELL: u8 = 0;
pub(super) const QUERY_COMMAND: u8 = 1;
