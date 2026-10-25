use super::collector::Collector;
use core::cell::RefCell;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll};
use core::time::Duration;
use std::time::Instant;
std::thread_local! {
    static ACTIVE: RefCell<Option<Span>> = const { RefCell::new(None) };
}
struct Span {
    collector: Collector,
    excluded: Duration,
}
struct Guard {
    previous: Option<Span>,
    started: Instant,
    worker: bool,
}
impl Guard {
    fn enter(collector: Collector, worker: bool) -> Self {
        Self {
            previous: ACTIVE.replace(Some(Span {
                collector,
                excluded: Duration::ZERO,
            })),
            started: Instant::now(),
            worker,
        }
    }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let elapsed = self.started.elapsed();
        let span = ACTIVE
            .replace(self.previous.take())
            .unwrap_or_else(|| panic!("missing processing span"));
        let processing = elapsed
            .checked_sub(span.excluded)
            .unwrap_or_else(|| panic!("excluded time exceeds processing span"));
        span.collector.record(processing, self.worker);
    }
}
pub(super) fn current() -> Option<Collector> {
    ACTIVE.with_borrow(|active| active.as_ref().map(|span| span.collector.clone()))
}
pub(crate) fn blocking<T>(collector: Option<Collector>, operation: impl FnOnce() -> T) -> T {
    let _guard = collector.map(|value| Guard::enter(value, true));
    operation()
}
pub(crate) fn exclude<T>(operation: impl FnOnce() -> T) -> T {
    if current().is_none() {
        return operation();
    }
    let started = Instant::now();
    let output = operation();
    ACTIVE.with_borrow_mut(|active| {
        let span = active
            .as_mut()
            .unwrap_or_else(|| panic!("missing excluded processing span"));
        span.excluded = span
            .excluded
            .checked_add(started.elapsed())
            .unwrap_or_else(|| panic!("excluded processing time overflow"));
    });
    output
}
pub(super) struct Tracked<F> {
    future: Pin<Box<F>>,
    collector: Collector,
}
impl<F> Tracked<F> {
    pub(super) fn new(future: F, collector: Collector) -> Self {
        Self {
            future: Box::pin(future),
            collector,
        }
    }
}
impl<F: Future> Future for Tracked<F> {
    type Output = F::Output;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let tracked = self.get_mut();
        let _guard = Guard::enter(tracked.collector.clone(), false);
        tracked.future.as_mut().poll(cx)
    }
}
