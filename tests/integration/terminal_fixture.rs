use core::time::Duration;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use std::{io::Read as _, sync::mpsc, thread};
pub(super) fn run(command: CommandBuilder) -> String {
    let pair = native_pty_system().openpty(PtySize::default()).unwrap();
    let mut child = pair.slave.spawn_command(command).unwrap();
    let mut killer = child.clone_killer();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let output_worker = thread::spawn(move || {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).unwrap();
        bytes
    });
    let (sender, receiver) = mpsc::channel();
    let wait_worker = thread::spawn(move || {
        sender.send(child.wait()).unwrap();
    });
    let result = receiver.recv_timeout(Duration::from_secs(20));
    if result.is_err() {
        killer.kill().unwrap();
    }
    drop(killer);
    drop(pair.slave);
    drop(pair.master);
    wait_worker.join().unwrap();
    let bytes = output_worker.join().unwrap();
    let text = String::from_utf8_lossy(&bytes);
    let output = fast_strip_ansi::strip_ansi_string(&text).into_owned();
    let status = result
        .unwrap_or_else(|error| panic!("fixture failed to finish: {error}; {output}"))
        .unwrap();
    assert!(status.success(), "{status:?}: {output}");
    output
}
