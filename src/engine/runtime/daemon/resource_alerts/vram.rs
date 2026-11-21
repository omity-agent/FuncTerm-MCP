use super::sampling::{usage_percent, warn_if_high};
use core::num::NonZeroU64;
use gpuviewer_core::{BackendError, DeviceId, GpuBackend};
pub(super) struct VramSampler {
    sources: Vec<Source>,
}
impl VramSampler {
    pub(super) fn new() -> Self {
        #[cfg(windows)]
        let sources = [Source::connect(gpuviewer_core::wddm::WddmBackend::init())];
        #[cfg(target_os = "linux")]
        let sources = [
            Source::connect(gpuviewer_core::nvidia::NvidiaBackend::init()),
            Source::connect(gpuviewer_core::amd::AmdBackend::init()),
        ];
        Self {
            sources: sources.into_iter().flatten().collect(),
        }
    }
    pub(super) fn sample(&mut self, threshold: u8, warnings: &mut String) {
        for source in &mut self.sources {
            for adapter in &source.adapters {
                match source.backend.refresh_dynamic(&adapter.id) {
                    Ok(sample) => {
                        if let Some(used) = sample.mem_used_bytes {
                            let percent = usage_percent(used, adapter.total);
                            warn_if_high(warnings, &adapter.label, percent, threshold);
                        }
                    }
                    Err(error) => {
                        tracing::debug!(%error, adapter = %adapter.id, "VRAM sampling failed");
                    }
                }
            }
        }
    }
}
struct Source {
    backend: Box<dyn GpuBackend>,
    adapters: Vec<Adapter>,
}
impl Source {
    fn connect(candidate: Result<impl GpuBackend + 'static, BackendError>) -> Option<Self> {
        let mut backend = match candidate {
            Ok(available) => available,
            Err(error) => {
                tracing::debug!(%error, "GPU metrics backend is unavailable");
                return None;
            }
        };
        let adapters = backend
            .devices()
            .into_iter()
            .filter_map(|id| match backend.static_info(&id) {
                Ok(info) => {
                    let total = NonZeroU64::new(info.mem_total_bytes?)?;
                    Some(Adapter {
                        label: format!("VRAM ({} at {id})", info.name),
                        id,
                        total,
                    })
                }
                Err(error) => {
                    tracing::debug!(%error, adapter = %id, "GPU memory capacity is unavailable");
                    None
                }
            })
            .collect();
        Some(Self {
            backend: Box::new(backend),
            adapters,
        })
    }
}
struct Adapter {
    id: DeviceId,
    label: String,
    total: NonZeroU64,
}
