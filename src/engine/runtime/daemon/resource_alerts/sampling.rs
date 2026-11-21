use super::{AlertSample, ResourceAlertSettings, SAMPLE_INTERVAL, append_paragraph};
use core::convert::Infallible;
use core::num::NonZeroU64;
use parking_lot::RwLock;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::time::Instant;
use sysinfo::{MINIMUM_CPU_UPDATE_INTERVAL, MemoryRefreshKind, System};
struct Sampler {
    system: Option<System>,
    #[cfg(any(windows, target_os = "linux"))]
    vram: super::vram::VramSampler,
}
impl Sampler {
    fn new() -> Self {
        let system = match System::new() {
            Ok(mut system) => {
                system.refresh_cpu_usage();
                Some(system)
            }
            Err(error) => {
                tracing::warn!("CPU and RAM metrics are unavailable");
                tracing::debug!(%error, "system metrics initialization failure details");
                None
            }
        };
        Self {
            system,
            #[cfg(any(windows, target_os = "linux"))]
            vram: super::vram::VramSampler::new(),
        }
    }
    fn sample(&mut self, settings: ResourceAlertSettings) -> String {
        let mut warnings = String::new();
        if let Some(system) = self.system.as_mut() {
            system.refresh_cpu_usage();
            if !system.cpus().is_empty() {
                warn_if_high(
                    &mut warnings,
                    "CPU",
                    f64::from(system.global_cpu_usage()),
                    settings.cpu_percent,
                );
            }
            system.refresh_memory_specifics(MemoryRefreshKind::nothing().with_ram());
            if let Some(total) = NonZeroU64::new(system.total_memory()) {
                warn_if_high(
                    &mut warnings,
                    "RAM",
                    usage_percent(system.used_memory(), total),
                    settings.ram_percent,
                );
            }
        }
        #[cfg(any(windows, target_os = "linux"))]
        self.vram.sample(settings.vram_percent, &mut warnings);
        warnings
    }
}
pub(super) fn watch(
    settings: ResourceAlertSettings,
    output: &RwLock<Option<AlertSample>>,
    stop: &Receiver<Infallible>,
) {
    let mut sampler = Sampler::new();
    let interval = SAMPLE_INTERVAL.max(MINIMUM_CPU_UPDATE_INTERVAL);
    while stop.recv_timeout(interval) == Err(RecvTimeoutError::Timeout) {
        let sampled_at = Instant::now();
        let warnings = sampler.sample(settings);
        *output.write() = Some(AlertSample {
            sampled_at,
            warnings,
        });
    }
}
pub(super) fn warn_if_high(warnings: &mut String, name: &str, usage: f64, threshold: u8) {
    if usage.is_finite() && usage >= f64::from(threshold) {
        append_paragraph(
            warnings,
            &format!("Host resource warning: {name} usage {usage:.1}% (threshold {threshold}%)."),
        );
    }
}
#[expect(
    clippy::as_conversions,
    reason = "f64 represents these byte counts with sufficient precision for a usage percentage"
)]
pub(super) fn usage_percent(used: u64, total: NonZeroU64) -> f64 {
    100.0 * used.min(total.get()) as f64 / total.get() as f64
}
