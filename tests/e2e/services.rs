#[path = "services/client_context.rs"]
mod client_context;
#[path = "services/closure.rs"]
mod closure;
#[cfg(windows)]
#[path = "services/console.rs"]
mod console;
#[path = "services/framing.rs"]
mod framing;
#[path = "services/gateway.rs"]
mod mcp;
