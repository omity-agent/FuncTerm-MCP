use super::capacity::DaemonPermit;
use super::command::{CLI_COMMAND_TIMEOUT, exe};
use super::process::ChildGuard;
use super::temp;
use core::cell::RefCell;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use std::io::BufRead as _;
use std::process::{Command, Stdio};
use std::thread;
static SERVICE_COUNTER: AtomicU64 = AtomicU64::new(0);
thread_local! { static ACTIVE_CLI_ENV : RefCell < Vec < (String , String) >> = const { RefCell :: new (Vec :: new ()) } ; }
pub(crate) struct TestGuard {
    daemon: ChildGuard,
    env: Vec<(String, String)>,
    service_name: String,
    http_address: String,
    previous_env: Vec<(String, String)>,
    _runtime: temp::TestRuntime,
    _permit: DaemonPermit,
}
impl TestGuard {
    pub(crate) fn env(&self) -> Vec<(String, String)> {
        self.env.clone()
    }
    pub(crate) fn http_address(&self) -> &str {
        &self.http_address
    }
    pub(crate) fn stop_daemon(&mut self) {
        self.daemon.terminate();
    }
    pub(crate) fn restart_daemon(&mut self) {
        self.stop_daemon();
        thread::sleep(Duration::from_millis(100));
        (self.daemon, self.http_address) = spawn_daemon(&self.env, &self.service_name);
    }
}
impl Drop for TestGuard {
    fn drop(&mut self) {
        let stopped = super::command::run_cli_with_env(&["daemon", "stop"], &self.env);
        if !stopped.status.success() {
            eprintln!(
                "failed to stop test daemon: {}",
                String::from_utf8_lossy(&stopped.stderr)
            );
        }
        set_active_env(&self.previous_env);
    }
}
#[cfg(windows)]
pub(crate) fn locked() -> TestGuard {
    locked_with_env(&[])
}
pub(crate) fn locked_with_env(extra_env: &[(&str, &str)]) -> TestGuard {
    let permit = DaemonPermit::acquire();
    let runtime = temp::TestRuntime::new();
    let service_name = unique_service_name();
    let mut env = vec![
        (
            "FUNCTERM_DAEMON_SERVICE_NAME".to_owned(),
            service_name.clone(),
        ),
        (
            "FUNCTERM_MCP_HTTP_BIND".to_owned(),
            "127.0.0.1:0".to_owned(),
        ),
    ];
    env.extend(extra_env.iter().map(|pair| {
        let key = pair.0.to_owned();
        let value = pair.1.to_owned();
        (key, value)
    }));
    env.extend(runtime.environment());
    let (child, http_address) = spawn_daemon(&env, &service_name);
    let previous_env = active_env();
    set_active_env(&env);
    TestGuard {
        daemon: child,
        env,
        service_name,
        http_address,
        previous_env,
        _runtime: runtime,
        _permit: permit,
    }
}
pub(crate) fn apply_active_env(command: &mut Command) {
    ACTIVE_CLI_ENV.with(|env| super::test_environment::apply(command, &env.borrow()));
}
pub(crate) fn active_env() -> Vec<(String, String)> {
    ACTIVE_CLI_ENV.with(|env| env.borrow().clone())
}
pub(crate) fn set_active_env(env: &[(String, String)]) {
    ACTIVE_CLI_ENV.with(|active| active.replace(env.to_vec()));
}
fn unique_service_name() -> String {
    let unique = SERVICE_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("functerm/test/{}/{}", unique, std::process::id())
}
fn spawn_daemon(env: &[(String, String)], service_name: &str) -> (ChildGuard, String) {
    let mut command = Command::new(exe());
    command
        .arg("internal-run-daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit());
    apply_daemon_flags(&mut command);
    super::test_environment::apply(&mut command, env);
    command.env("FUNCTERM_DAEMON_READY_STDOUT", "1");
    let mut child = ChildGuard::spawn(&mut command).unwrap();
    let http_address = wait_for_daemon(child.child_mut(), service_name);
    (child, http_address)
}
fn wait_for_daemon(child: &mut std::process::Child, service_name: &str) -> String {
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    thread::spawn(move || {
        let mut line = String::new();
        let result = std::io::BufReader::new(stdout)
            .read_line(&mut line)
            .map(|bytes| (bytes, line));
        sender.send(result).unwrap();
    });
    let (bytes, line) = receiver
        .recv_timeout(CLI_COMMAND_TIMEOUT)
        .unwrap_or_else(|_| panic!("daemon did not become ready within {CLI_COMMAND_TIMEOUT:?}"))
        .unwrap();
    assert!(
        child.try_wait().unwrap().is_none(),
        "daemon exited before accepting connections"
    );
    assert!(bytes > 0, "daemon closed startup pipe for {service_name}");
    assert!(
        line.contains("Ready"),
        "daemon did not report readiness for {service_name}: {line}"
    );
    let reply: rmcp::serde_json::Value = rmcp::serde_json::from_str(&line).unwrap();
    reply
        .pointer("/Ready/http_address")
        .and_then(rmcp::serde_json::Value::as_str)
        .unwrap_or_else(|| panic!("daemon startup did not publish its HTTP address: {line}"))
        .to_owned()
}
#[cfg(windows)]
fn apply_daemon_flags(command: &mut Command) {
    use std::os::windows::process::CommandExt as _;
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    command.creation_flags(DETACHED_PROCESS);
}
#[cfg(not(windows))]
fn apply_daemon_flags(_command: &mut Command) {}
