use super::super::{shell_session::KeyboardWriteFailure, tab::Tab};
use crate::runtime::protocol::{KeyboardInput, ViewResult};
use alloc::sync::Arc;
use anyhow::Result;
use core::time::Duration;
impl Tab {
    pub(in crate::engine::runtime::session::manager) async fn manual_write(
        self: &Arc<Self>,
        input: KeyboardInput,
        wait_timeout: Duration,
    ) -> Result<ViewResult> {
        let target_tab = Arc::clone(self);
        let (session, delivery) = self
            .blocking
            .run(move || {
                let session = target_tab.live_session()?;
                if !session.is_alive()? {
                    target_tab.close_session(&session)?;
                    anyhow::bail!(
                        "tab id {} was generated, but its shell is gone",
                        target_tab.id()
                    );
                }
                session.refresh_choice()?;
                let delivery = session.write_keyboard_for_running_command(&input);
                Ok((session, delivery))
            })
            .await?;
        match delivery {
            Ok(revision) => {
                session.wait_for_output(revision, wait_timeout).await?;
                let observed_tab = Arc::clone(self);
                self.blocking
                    .run(move || {
                        if session.is_alive()? {
                            Ok(observed_tab.remember(&session)?.into_view(true))
                        } else {
                            observed_tab.close_session(&session)?;
                            Ok(observed_tab.snapshot_view())
                        }
                    })
                    .await
            }
            Err(
                error @ (KeyboardWriteFailure::IdlePrompt | KeyboardWriteFailure::CommandEnded),
            ) => Err(error.into()),
            Err(KeyboardWriteFailure::Write(error)) => {
                let failed_tab = Arc::clone(self);
                self.blocking
                    .run(move || failed_tab.close_session(&session))
                    .await?;
                Err(error)
            }
        }
    }
}
