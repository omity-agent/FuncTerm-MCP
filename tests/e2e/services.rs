#[cfg(test)]
#[path = "services/client_context.rs"]
mod client_context;
#[cfg(test)]
#[path = "services/closure.rs"]
mod closure;
#[cfg(all(test, windows))]
#[path = "services/console.rs"]
mod console;
#[cfg(test)]
#[path = "services/framing.rs"]
mod framing;
#[cfg(test)]
#[path = "services/gateway.rs"]
mod mcp;
