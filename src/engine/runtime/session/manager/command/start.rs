use super::super::tab::Tab;
use super::{ManagedCommand, SessionEnd};
use crate::runtime::protocol::RequestRejected;
use crate::runtime::session::records::{create_record, discard_record};
use alloc::sync::Arc;
use anyhow::Result;
impl Tab {
    pub(in crate::engine::runtime::session::manager) fn dispatch_command(
        self: &Arc<Self>,
        command_id: String,
        command_text: &str,
        separate_output: bool,
    ) -> Result<Arc<ManagedCommand>> {
        let session = self.live_session()?;
        session.ensure_idle(self.id())?;
        session.refresh_route()?;
        let frame = session.dispatch_frame(self.id())?;
        let choice = frame.choice;
        if separate_output && !choice.supports_separated_output() {
            return Err(RequestRejected::new(
                "NuShell does not support separate_output=true; use separate_output=false",
            )
            .into());
        }
        let record = create_record(session.command_root(), &command_id, choice)?;
        let capture = session
            .capture_command(&command_id, frame, separate_output)
            .inspect_err(|_error| discard_record(&record))?;
        let managed = Arc::new(ManagedCommand::new(
            command_id,
            frame,
            record,
            capture,
            Arc::clone(&self.tokenizer),
            session.command_start_timeout(),
        ));
        session.reserve(Arc::clone(&managed));
        if let Err(error) = session.write_invocation(
            frame,
            managed.id(),
            command_text,
            managed.record(),
            separate_output,
        ) {
            if let Err(cleanup) = self.end_session(&session, SessionEnd::Failed) {
                tracing::error!("failed to close the Tab after command submission failed");
                tracing::debug!(
                    error = format!("{cleanup:#}"),
                    "shell shutdown failure details"
                );
            }
            return Err(error);
        }
        self.supervise(
            session.terminal(),
            session.workspace_events(),
            session.tab_root().to_path_buf(),
            Arc::clone(&managed),
        );
        Ok(managed)
    }
}
