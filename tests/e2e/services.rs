#[cfg(test)]
#[path = "services/closure.rs"]
mod closure;
#[cfg(all(test, windows))]
#[path = "services/console.rs"]
mod console;
#[cfg(all(test, windows))]
#[path = "services/diagnostics.rs"]
mod diagnostics;
#[cfg(test)]
#[path = "services/framing.rs"]
mod framing;
#[cfg(test)]
#[path = "services/gateway.rs"]
mod mcp;
#[cfg(test)]
#[path = "services/recovery.rs"]
mod recovery;
#[cfg(all(test, windows))]
#[path = "services/throughput.rs"]
mod throughput;
