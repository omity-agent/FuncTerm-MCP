mod bootstrap;
use super::{DriverStartup, StartupContext};
use anyhow::Result;
pub(super) fn startup(context: StartupContext<'_>) -> Result<DriverStartup> {
    let script = context.startup_directory.join("python_repl.py");
    fs_err::write(&script, bootstrap::script(context)?)?;
    Ok(DriverStartup {
        args: vec![
            "-i".to_owned(),
            "-u".to_owned(),
            crate::text::path_text(&script, "Python REPL bootstrap path")?,
        ],
        env: Vec::new(),
    })
}
pub(super) fn interactive_arguments(arguments: &[std::ffi::OsString]) -> bool {
    arguments.iter().all(|argument| {
        argument
            .to_str()
            .is_some_and(|value| matches!(value, "-i" | "-u" | "-q"))
    })
}
