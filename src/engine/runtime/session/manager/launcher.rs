#[cfg(windows)]
mod host_wait;
mod spawn_builder;
mod startup;
use super::process;
use super::shell_session::ShellSession;
use crate::contract::DISPATCH_FILE;
use crate::engine::privileges::ShellIdentity;
use crate::runtime::config::Settings;
use crate::runtime::protocol::EnvironmentSnapshot;
use crate::runtime::session::observation::PathWatch;
use crate::runtime::session::terminal::{Terminal, start_reader};
use crate::runtime::temp;
use crate::shell::{ShellChoice, shims};
use alloc::sync::Arc;
use anyhow::{Context as _, Result};
use core::time::Duration;
use notify::RecursiveMode;
use parking_lot::Mutex;
use portable_pty::PtySize;
use std::path::Path;
use std::sync::OnceLock;
use tastty_core::TerminalSize;
pub(super) struct ShellLauncher {
    settings: Settings,
    generation_root: std::path::PathBuf,
    shim_dir: std::path::PathBuf,
    shim_ready: (OnceLock<()>, OnceLock<()>),
    #[cfg(unix)]
    runtime_root: std::path::PathBuf,
}
impl ShellLauncher {
    pub(super) fn new(settings: Settings) -> Result<Self> {
        let root = temp::daemon_root()?;
        temp::remove_stale_service_runtime(&root, &settings.daemon_service_name);
        let generation = runtime_generation();
        let generation_root =
            temp::generation_root(&root, &settings.daemon_service_name, &generation);
        let shim_dir = temp::shim_directory(&generation_root);
        Ok(Self {
            settings,
            generation_root,
            shim_dir,
            shim_ready: (OnceLock::new(), OnceLock::new()),
            #[cfg(unix)]
            runtime_root: root,
        })
    }
    pub(super) fn launch(
        &self,
        tab_id: &str,
        starting_directory: &Path,
        starting_shell: ShellChoice,
        load_profile: bool,
        elevated: bool,
        client_environment: &EnvironmentSnapshot,
    ) -> Result<Arc<ShellSession>> {
        let identity = ShellIdentity::resolve(elevated, &self.settings)?;
        let tab_root = temp::tab_root(&self.generation_root, tab_id);
        let shim_dir = self
            .shim_dir
            .join(if elevated { "elevated" } else { "normal" });
        fs_err::create_dir_all(&tab_root)?;
        fs_err::create_dir_all(&shim_dir)?;
        #[cfg(unix)]
        identity.prepare_workspace(
            &self.runtime_root,
            &self.generation_root,
            &tab_root,
            &shim_dir,
        )?;
        let ready = if elevated {
            &self.shim_ready.1
        } else {
            &self.shim_ready.0
        };
        ready.get_or_try_init(|| shims::ensure_directory(&shim_dir))?;
        let tab_state = temp::tab_state_directory(&tab_root);
        let command_root = temp::tab_commands_directory(&tab_root);
        fs_err::create_dir_all(&tab_state)?;
        fs_err::create_dir_all(&command_root)?;
        let command_watch = Arc::new(PathWatch::new(
            &command_root,
            RecursiveMode::Recursive,
            crate::runtime::session::records::lifecycle_event,
        )?);
        let managed_environment = shims::environment(
            &self.settings,
            &tab_root,
            &shim_dir,
            starting_shell,
            client_environment,
            starting_directory,
        )?;
        let executable =
            crate::text::path_text(&managed_environment.executable, "executable path")?;
        let mut variables = managed_environment.variables;
        variables.push((shims::TAB_ID_ENV.into(), tab_id.into()));
        let startup = starting_shell.startup(
            starting_directory,
            &tab_root,
            load_profile,
            self.settings.shell_ignore_style,
            &EnvironmentSnapshot::from_variables(variables),
        )?;
        let screen = Arc::new(Terminal::new(
            TerminalSize {
                rows: self.settings.terminal_rows,
                cols: self.settings.terminal_cols,
            },
            &self.settings.terminal_model_title,
        )?);
        let ready_file = startup.ready_file.clone();
        let spawned = spawn_builder::launch(
            &executable,
            startup,
            starting_directory,
            &identity,
            PtySize {
                rows: self.settings.terminal_rows,
                cols: self.settings.terminal_cols,
                pixel_width: 0,
                pixel_height: 0,
            },
        )?;
        let process_tree = process::ProcessTree::new();
        let mut child = spawned.child;
        let retained_slave = spawned.slave;
        if let Err(error) = process_tree
            .attach(child.as_ref())
            .context("failed to guard shell process tree")
        {
            process::cleanup(child.as_mut(), "unregistered shell child");
            return Err(error);
        }
        let writer = Arc::new(Mutex::new(spawned.writer));
        let startup_timeout =
            Duration::try_from_secs_f64(self.settings.shell_startup_timeout_seconds)
                .context("shell_startup_timeout_seconds must be finite and non-negative")?;
        let startup_events = startup::StartupEvents::new();
        let reader_worker = match start_reader(
            Arc::clone(&screen),
            Arc::clone(&writer),
            spawned.reader,
            startup_events.reader_exit_notifier(),
        ) {
            Ok(worker) => worker,
            Err(error) => {
                process::cleanup(child.as_mut(), "unregistered shell child");
                return Err(error).context("failed to start pty reader");
            }
        };
        let initialized = startup_events
            .wait(&mut child, &ready_file, &screen, startup_timeout)
            .with_context(|| {
                format!(
                    "failed to initialize {} at {executable} with shim directory {}",
                    starting_shell.canonical_name(),
                    shim_dir.display(),
                )
            });
        if let Err(error) = initialized {
            process::cleanup(child.as_mut(), "unregistered shell child");
            drop(retained_slave);
            drop(spawned.master);
            process::join_reader(reader_worker, "pty reader thread");
            return Err(error);
        }
        let active_shell_file = tab_state.join("active-shell.txt");
        Ok(Arc::new(
            ShellSession::builder()
                .choice(starting_shell)
                .cwd(starting_directory.to_path_buf())
                .writer(writer)
                .screen(screen)
                .command_root(command_root)
                .command_watch(command_watch)
                .dispatch_file(tab_state.join(DISPATCH_FILE))
                .active_shell_file(active_shell_file)
                .command_start_timeout(startup_timeout)
                .process_tree(process_tree)
                .child(child)
                .slave(retained_slave)
                .reader(Some(reader_worker))
                .build(),
        ))
    }
}
fn runtime_generation() -> String {
    format!("{}-{}", std::process::id(), nanoid::nanoid!())
}
