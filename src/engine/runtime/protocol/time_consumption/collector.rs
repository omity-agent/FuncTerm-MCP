use super::suspension;
use alloc::sync::Arc;
use core::future::Future;
use core::sync::atomic::{AtomicU64, Ordering};
use core::time::Duration;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) struct ProcessingTime {
    pub(crate) poll_ns: u64,
    pub(crate) blocking_ns: u64,
}
impl ProcessingTime {
    pub(crate) fn merge_current(&self) {
        if let Some(collector) = current() {
            collector.merge(self);
        }
    }
}
#[derive(Clone, Default)]
pub(crate) struct Collector {
    polls: Arc<AtomicU64>,
    workers: Arc<AtomicU64>,
}
impl Collector {
    pub(super) fn record(&self, elapsed: Duration, worker: bool) {
        let nanos = u64::try_from(elapsed.as_nanos())
            .unwrap_or_else(|error| panic!("processing duration overflow: {error}"));
        add(if worker { &self.workers } else { &self.polls }, nanos);
    }
    pub(crate) fn merge(&self, measurement: &ProcessingTime) {
        add(&self.polls, measurement.poll_ns);
        add(&self.workers, measurement.blocking_ns);
    }
    fn snapshot(&self) -> ProcessingTime {
        ProcessingTime {
            poll_ns: self.polls.load(Ordering::Relaxed),
            blocking_ns: self.workers.load(Ordering::Relaxed),
        }
    }
}
fn add(counter: &AtomicU64, nanos: u64) {
    counter
        .try_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(nanos)
        })
        .unwrap_or_else(|value| panic!("processing time counter overflow: {value}"));
}
pub(crate) fn current() -> Option<Collector> {
    suspension::current()
}
pub(crate) async fn capture<F>(enabled: bool, future: F) -> (F::Output, Option<ProcessingTime>)
where
    F: Future,
{
    if !enabled {
        return (future.await, None);
    }
    let collector = Collector::default();
    let output = suspension::Tracked::new(future, collector.clone()).await;
    (output, Some(collector.snapshot()))
}
