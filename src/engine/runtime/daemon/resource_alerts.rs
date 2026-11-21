mod sampling;
#[cfg(any(windows, target_os = "linux"))]
mod vram;
use crate::runtime::protocol::{Payload, ViewResult};
use alloc::sync::Arc;
use anyhow::{Context as _, Result, ensure};
use core::convert::Infallible;
use core::str::FromStr;
use core::time::Duration;
use parking_lot::RwLock;
use serde::Deserialize;
use std::env::{self, VarError};
use std::sync::mpsc;
use std::thread;
use std::time::Instant;
const SAMPLE_INTERVAL: Duration = Duration::from_secs(1);
const MAX_SAMPLE_AGE: Duration = Duration::from_secs(3);
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct ResourceAlertSettings {
    enabled: bool,
    cpu_percent: u8,
    ram_percent: u8,
    vram_percent: u8,
}
impl ResourceAlertSettings {
    pub(crate) fn apply_environment(&mut self) -> Result<()> {
        read_override("FUNCTERM_RESOURCE_ALERTS", &mut self.enabled)?;
        for (name, threshold) in [
            ("FUNCTERM_RESOURCE_CPU_PERCENT", &mut self.cpu_percent),
            ("FUNCTERM_RESOURCE_RAM_PERCENT", &mut self.ram_percent),
            ("FUNCTERM_RESOURCE_VRAM_PERCENT", &mut self.vram_percent),
        ] {
            read_override(name, threshold)?;
            ensure!(*threshold <= 100, "{name} must be an integer in 0..=100");
        }
        Ok(())
    }
}
fn read_override<T>(name: &str, value: &mut T) -> Result<()>
where
    T: FromStr,
    T::Err: core::error::Error + Send + Sync + 'static,
{
    match env::var(name) {
        Ok(setting) => {
            *value = setting
                .parse()
                .with_context(|| format!("invalid value in {name}"))?;
        }
        Err(VarError::NotPresent) => {}
        Err(error) => return Err(error).with_context(|| format!("failed to read {name}")),
    }
    Ok(())
}
struct AlertSample {
    sampled_at: Instant,
    warnings: String,
}
pub(super) struct ResourceMonitor {
    latest: Arc<RwLock<Option<AlertSample>>>,
    _sampler_lifetime: mpsc::Sender<Infallible>,
}
impl ResourceMonitor {
    pub(super) fn start(settings: ResourceAlertSettings) -> Option<Self> {
        if !settings.enabled {
            return None;
        }
        let latest = Arc::new(RwLock::new(None));
        let output = Arc::clone(&latest);
        let (sampler_lifetime, stop) = mpsc::channel();
        if let Err(error) = thread::Builder::new()
            .name("functerm-resources".to_owned())
            .spawn(move || sampling::watch(settings, &output, &stop))
        {
            tracing::warn!("could not start resource monitoring");
            tracing::debug!(%error, "resource monitor startup failure details");
            return None;
        }
        Some(Self {
            latest,
            _sampler_lifetime: sampler_lifetime,
        })
    }
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "match ergonomics borrow the note without moving or rebuilding the payload"
    )]
    pub(super) fn annotate(&self, payload: &mut Payload) {
        let (Payload::KeyboardWritten { view }
        | Payload::CommandAccepted {
            view,
            command_id: _,
        }
        | Payload::View(view)) = payload
        else {
            return;
        };
        if let Some(sample) = self.latest.read().as_ref()
            && sample.sampled_at.elapsed() <= MAX_SAMPLE_AGE
            && !sample.warnings.is_empty()
        {
            let note = match view {
                ViewResult::Tab {
                    note,
                    shell: _,
                    screen: _,
                }
                | ViewResult::Command {
                    note,
                    shell: _,
                    command: _,
                    end_reason: _,
                } => note,
            };
            append_paragraph(note, &sample.warnings);
        }
    }
}
fn append_paragraph(note: &mut String, text: &str) {
    if !note.is_empty() && !note.ends_with('\n') {
        note.push('\n');
    }
    note.push_str(text);
}
