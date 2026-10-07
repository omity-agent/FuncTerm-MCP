use super::instrumentation;
use crate::contract::{
    COMMAND_FILE, COMMAND_INPUT_DIRECTORY, COMMAND_STATE_DIRECTORY, SESSION_STATE_DIRECTORY,
};
use crate::shell::quote;
use anyhow::Result;
use std::path::Path;
pub(in crate::shell) fn prepare_nushell_command(
    directory: &Path,
    session_root: &Path,
) -> Result<()> {
    let input = directory.join(COMMAND_INPUT_DIRECTORY);
    let session_state = session_root.join(SESSION_STATE_DIRECTORY);
    fs_err::create_dir_all(&session_state)?;
    let declarations = session_state.join("nushell-declarations.nu");
    crate::publication::write_once(&declarations, b"")?;
    let script = input.join("command.nu");
    fs_err::copy(input.join(COMMAND_FILE), &script)?;
    fs_err::write(input.join("functerm-env.nu"), b"")?;
    let config = format!(
        r#"$env.config.show_banner = false
$env.config.use_ansi_coloring = false
$env.config.shell_integration = {{ osc2: false, osc7: false, osc8: false, osc9_9: false, osc133: false, osc633: false, reset_application_mode: false }}
$env.config.history.file_format = "plaintext"
$env.config.history.max_size = 0
$env.config.history.sync_on_enter = false
let @VAR_env_state_file@ = {environment}
if ($@VAR_env_state_file@ | path exists) {{ load-env (open --raw $@VAR_env_state_file@ | from nuon) }}
let @VAR_config_state_file@ = {configuration}
if ($@VAR_config_state_file@ | path exists) {{ $env.config = ($env.config | merge (open --raw $@VAR_config_state_file@ | from nuon)) }}
if not ($env.FUNCTERM_SHIM_DIR? | is-empty) {{ $env.PATH = ($env.PATH | where {{|@VAR_entry@| $@VAR_entry@ != $env.FUNCTERM_SHIM_DIR }} | prepend $env.FUNCTERM_SHIM_DIR) }}
$env.config.hooks.display_output = {{ save_nushell_state {cwd} {environment} {configuration} {declarations}; $in | table }}
@NUSHELL_STATE_FUNCTIONS@
@NUSHELL_EXIT_HOOK@
"#,
        environment = quote::nushell_path(&session_state.join("nushell-env.nuon"))?,
        configuration = quote::nushell_path(&session_state.join("nushell-config.nuon"))?,
        cwd = quote::nushell_path(
            &directory
                .join(COMMAND_STATE_DIRECTORY)
                .join("nushell-cwd.txt")
        )?,
        declarations = quote::nushell_path(&declarations)?,
    );
    fs_err::write(
        input.join("functerm-config.nu"),
        instrumentation::render_worker(&config),
    )?;
    fs_err::write(
        input.join("nushell-launch.txt"),
        format!(
            "source {}; source {}",
            quote::nushell_path(&declarations)?,
            quote::nushell_path(&script)?
        ),
    )?;
    Ok(())
}
