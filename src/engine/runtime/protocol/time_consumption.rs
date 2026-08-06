use core::time::Duration;
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
        scaled(nanoseconds, NANOS_PER_SECOND, "s")
    } else {
        scaled(nanoseconds, NANOS_PER_MINUTE, "min")
    }
}
pub(super) fn milliseconds(duration: Duration) -> String {
    scaled(duration.as_nanos(), NANOS_PER_MILLISECOND, "ms")
}
#[expect(
    clippy::integer_division,
    reason = "duration units require exact integer quotient and remainder formatting"
)]
#[expect(
    clippy::integer_division_remainder_used,
    reason = "duration units require exact integer quotient and remainder formatting"
)]
fn scaled(nanoseconds: u128, nanoseconds_per_unit: u128, suffix: &str) -> String {
    let whole = nanoseconds / nanoseconds_per_unit;
    let remainder = nanoseconds % nanoseconds_per_unit;
    let rounded_numerator = remainder * DECIMAL_SCALE + nanoseconds_per_unit / 2;
    let fraction = rounded_numerator / nanoseconds_per_unit;
    let (rounded_whole, normalized_fraction) = if fraction == DECIMAL_SCALE {
        (whole + 1, 0)
    } else {
        (whole, fraction)
    };
    format!("{rounded_whole}.{normalized_fraction:0DECIMAL_WIDTH$}{suffix}")
}
