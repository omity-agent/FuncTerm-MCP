use core::time::Duration;
mod collector;
mod suspension;
pub(crate) use collector::{ProcessingTime, capture, current};
pub(crate) use suspension::{blocking, exclude};
const NANOS_PER_MILLISECOND: u128 = 1_000_000;
const NANOS_PER_SECOND: u128 = 1_000_000_000;
const NANOS_PER_MINUTE: u128 = 60 * NANOS_PER_SECOND;
const DECIMAL_SCALE: u128 = 100;
const DECIMAL_WIDTH: usize = 2;
pub(super) fn adaptive(duration: Duration) -> String {
    let nanoseconds = duration.as_nanos();
    if nanoseconds < NANOS_PER_SECOND {
        milliseconds(duration)
    } else if nanoseconds < NANOS_PER_MINUTE {
        scaled(duration, NANOS_PER_SECOND, "s")
    } else {
        scaled(duration, NANOS_PER_MINUTE, "min")
    }
}
pub(super) fn milliseconds(duration: Duration) -> String {
    scaled(duration, NANOS_PER_MILLISECOND, "ms")
}
#[expect(
    clippy::integer_division,
    reason = "round the entire Duration range to hundredths without floating-point precision loss"
)]
fn scaled(duration: Duration, nanoseconds_per_unit: u128, suffix: &str) -> String {
    let rounded =
        (duration.as_nanos() * DECIMAL_SCALE + nanoseconds_per_unit / 2) / nanoseconds_per_unit;
    let whole = rounded / DECIMAL_SCALE;
    let fraction = rounded % DECIMAL_SCALE;
    format!("{whole}.{fraction:0DECIMAL_WIDTH$}{suffix}")
}
