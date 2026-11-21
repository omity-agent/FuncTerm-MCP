use super::resource_alerts::ResourceMonitor;
use crate::runtime::config::Settings;
use crate::runtime::protocol::{Payload, Request};
use crate::runtime::session::Manager;
use alloc::sync::Arc;
use anyhow::{Result, bail};
pub(crate) struct Dispatcher {
    manager: Arc<Manager>,
    resources: Option<ResourceMonitor>,
}
impl Dispatcher {
    pub(super) fn new(settings: Settings) -> Result<Self> {
        let resource_settings = settings.resource_alerts;
        let manager = Arc::new(Manager::new(settings)?);
        Ok(Self {
            manager,
            resources: ResourceMonitor::start(resource_settings),
        })
    }
    pub(super) async fn shutdown(&self) -> Result<()> {
        self.manager.shutdown().await
    }
    pub(crate) async fn execute(&self, request: Request) -> Result<Payload> {
        let manager = &self.manager;
        let mut payload = match request {
            Request::Ping => Payload::Pong,
            Request::Shutdown => bail!("shutdown requests require daemon lifecycle control"),
            Request::Close { tab_id } => {
                manager.close(&tab_id).await?;
                Payload::TabClosed { tab_id }
            }
            Request::NewTab {
                starting_directory,
                starting_shell,
                load_profile,
                elevated,
                environment,
            } => {
                let tab_id = manager
                    .new_tab(
                        starting_directory,
                        starting_shell,
                        load_profile,
                        elevated,
                        environment,
                    )
                    .await?;
                Payload::TabCreated { tab_id }
            }
            Request::ManualWrite {
                tab_id,
                input,
                wait_timeout,
            } => {
                let view = manager.manual_write(&tab_id, input, wait_timeout).await?;
                Payload::KeyboardWritten { view }
            }
            Request::Run {
                tab_id,
                command,
                separate_output,
                wait_timeout,
            } => {
                let (command_id, view) = manager
                    .run(&tab_id, &command, separate_output, wait_timeout)
                    .await?;
                Payload::CommandAccepted { command_id, view }
            }
            Request::View {
                id,
                wait_timeout,
                start_token,
            } => Payload::View(manager.view(&id, wait_timeout, start_token).await?),
        };
        if let Some(resources) = self.resources.as_ref() {
            resources.annotate(&mut payload);
        }
        Ok(payload)
    }
}
