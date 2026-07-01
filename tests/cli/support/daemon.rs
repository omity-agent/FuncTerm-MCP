use super::command::{CLI_COMMAND_TIMEOUT, exe};
use super::process::ChildGuard;
#[path = "../../../src/runtime/iceoryx/shared.rs"]
mod iceoryx_shared;
#[path = "../../../src/runtime/protocol/wire.rs"]
mod protocol_wire;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use iceoryx2::prelude::*;
use std::process::{Command, Stdio};
use std::sync::{Mutex, MutexGuard};
use std::thread;
use std::time::Instant;
static CLI_TEST_LOCK: Mutex<()> = Mutex::new(());
static ACTIVE_CLI_ENV: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());
static SERVICE_COUNTER: AtomicU64 = AtomicU64::new(0);
pub(crate) struct TestGuard {
    _daemon: ChildGuard,
    _lock: MutexGuard<'static, ()>,
}
#[expect(
    clippy::missing_trait_methods,
    reason = "Drop only needs the regular destructor for this test guard"
)]
impl Drop for TestGuard {
    fn drop(&mut self) {
        ACTIVE_CLI_ENV
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }
}
pub(crate) fn locked() -> TestGuard {
    locked_with_env(&[])
}
pub(crate) fn locked_with_env(extra_env: &[(&str, &str)]) -> TestGuard {
    let guard = CLI_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let service_name = unique_service_name();
    let mut env = vec![(
        "SHELL_MCP_PTY_DAEMON_SERVICE_NAME".to_owned(),
        service_name.clone(),
    )];
    env.extend(extra_env.iter().map(|pair| {
        let key = pair.0.to_owned();
        let value = pair.1.to_owned();
        (key, value)
    }));
    let mut command = Command::new(exe());
    command
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    apply_daemon_flags(&mut command);
    apply_env(&mut command, &env);
    {
        let mut active = ACTIVE_CLI_ENV
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        *active = env;
    }
    let mut child = ChildGuard::new(command.spawn().unwrap());
    wait_for_daemon(&mut child, &service_name);
    TestGuard {
        _daemon: child,
        _lock: guard,
    }
}
pub(crate) fn apply_active_env(command: &mut Command) {
    let env = ACTIVE_CLI_ENV
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    apply_env(command, &env);
}
fn unique_service_name() -> String {
    let unique = SERVICE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("shell_mcp_pty/test/{}/{}", unique, std::process::id())
}
fn wait_for_daemon(child: &mut ChildGuard, service_name: &str) {
    let start = Instant::now();
    loop {
        assert!(
            child.is_running(),
            "daemon exited before accepting connections"
        );
        if has_daemon_server(service_name) {
            return;
        }
        assert!(
            start.elapsed() < CLI_COMMAND_TIMEOUT,
            "daemon did not become ready within {CLI_COMMAND_TIMEOUT:?}"
        );
        thread::sleep(Duration::from_millis(50));
    }
}
fn has_daemon_server(service_name: &str) -> bool {
    let Ok(config) = iceoryx_shared::config() else {
        return false;
    };
    let Ok(node) = NodeBuilder::new()
        .config(&config)
        .create::<ipc_threadsafe::Service>()
    else {
        return false;
    };
    let Ok(service) = node
        .service_builder(&service_name.try_into().unwrap())
        .request_response::<[u8], [u8]>()
        .request_user_header::<protocol_wire::RequestHeader>()
        .response_user_header::<protocol_wire::ResponseHeader>()
        .open_or_create()
    else {
        return false;
    };
    service.dynamic_config().number_of_servers() > 0
}
fn apply_env(command: &mut Command, env: &[(String, String)]) {
    for pair in env {
        command.env(&pair.0, &pair.1);
    }
}
#[cfg(windows)]
fn apply_daemon_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    command.creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}
#[cfg(not(windows))]
fn apply_daemon_flags(_command: &mut Command) {}
