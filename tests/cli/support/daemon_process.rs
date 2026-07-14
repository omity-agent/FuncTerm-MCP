use super::command::{CLI_COMMAND_TIMEOUT, exe};
use super::process::ChildGuard;
use super::temp;
use core::cell::RefCell;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use std::io::BufRead as _;
use std::process::{Command, Stdio};
use std::sync::{Condvar, Mutex};
use std::thread;
const MAX_PARALLEL_DAEMONS: usize = 2;
static SERVICE_COUNTER: AtomicU64 = AtomicU64::new(0);
static DAEMON_SLOTS: Mutex<SlotState> = Mutex::new(SlotState { active: 0 });
static DAEMON_SLOT_AVAILABLE: Condvar = Condvar::new();
thread_local! { static ACTIVE_CLI_ENV : RefCell < Vec < (String , String) >> = const { RefCell :: new (Vec :: new ()) } ; }
pub(crate) struct TestGuard {
    daemon: ChildGuard,
    env: Vec<(String, String)>,
    service_name: String,
    _slot: DaemonSlot,
}
struct DaemonSlot;
struct SlotState {
    active: usize,
}
impl DaemonSlot {
    fn acquire() -> Self {
        let mut active = DAEMON_SLOTS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        while active.active >= MAX_PARALLEL_DAEMONS {
            active = DAEMON_SLOT_AVAILABLE
                .wait(active)
                .unwrap_or_else(std::sync::PoisonError::into_inner);
        }
        active.active += 1;
        Self
    }
}
impl Drop for DaemonSlot {
    fn drop(&mut self) {
        {
            let mut active = DAEMON_SLOTS
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            active.active -= 1;
        }
        DAEMON_SLOT_AVAILABLE.notify_one();
    }
}
impl TestGuard {
    pub(crate) fn env(&self) -> Vec<(String, String)> {
        self.env.clone()
    }
    pub(crate) fn stop_daemon(&mut self) {
        self.daemon.terminate();
    }
    pub(crate) fn restart_daemon(&mut self) {
        self.stop_daemon();
        thread::sleep(Duration::from_millis(100));
        self.daemon = spawn_daemon(&self.env, &self.service_name);
    }
}
impl Drop for TestGuard {
    fn drop(&mut self) {
        ACTIVE_CLI_ENV.with(|env| env.borrow_mut().clear());
    }
}
#[cfg(windows)]
pub(crate) fn locked() -> TestGuard {
    locked_with_env(&[])
}
pub(crate) fn locked_with_env(extra_env: &[(&str, &str)]) -> TestGuard {
    let slot = DaemonSlot::acquire();
    let service_name = unique_service_name();
    let mut env = vec![(
        "FUNCTERM_DAEMON_SERVICE_NAME".to_owned(),
        service_name.clone(),
    )];
    env.extend(extra_env.iter().map(|pair| {
        let key = pair.0.to_owned();
        let value = pair.1.to_owned();
        (key, value)
    }));
    env.extend(temp_environment());
    set_active_env(&env);
    let child = spawn_daemon(&env, &service_name);
    TestGuard {
        daemon: child,
        env,
        service_name,
        _slot: slot,
    }
}
pub(crate) fn apply_active_env(command: &mut Command) {
    ACTIVE_CLI_ENV.with(|env| apply_env(command, &env.borrow()));
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
fn temp_environment() -> Vec<(String, String)> {
    let text = temp::temp_root().to_string_lossy().into_owned();
    platform_temp_environment(text)
}
#[cfg(windows)]
fn platform_temp_environment(text: String) -> Vec<(String, String)> {
    vec![("TMP".to_owned(), text.clone()), ("TEMP".to_owned(), text)]
}
#[cfg(not(windows))]
fn platform_temp_environment(text: String) -> Vec<(String, String)> {
    vec![("TMPDIR".to_owned(), text)]
}
fn spawn_daemon(env: &[(String, String)], service_name: &str) -> ChildGuard {
    let mut command = Command::new(exe());
    command
        .arg("daemon")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    apply_daemon_flags(&mut command);
    apply_env(&mut command, env);
    command.env("FUNCTERM_DAEMON_READY_STDOUT", "1");
    let mut child = command.spawn().unwrap();
    wait_for_daemon(&mut child, service_name);
    ChildGuard::new(child)
}
fn wait_for_daemon(child: &mut std::process::Child, service_name: &str) {
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
    command.creation_flags(DETACHED_PROCESS);
}
#[cfg(not(windows))]
fn apply_daemon_flags(_command: &mut Command) {}
