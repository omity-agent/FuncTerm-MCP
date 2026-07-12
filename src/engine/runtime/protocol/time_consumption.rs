use core::time::Duration;
const NANOS_PER_MILLISECOND: u128 = 1_000_000;
const NANOS_PER_SECOND: u128 = 1_000_000_000;
const NANOS_PER_MINUTE: u128 = 60 * NANOS_PER_SECOND;
const MILLISECOND_SCALE: u128 = 1_000_000;
const DISPLAY_SCALE: u128 = 1_000_000_000;
pub(super) fn adaptive(duration: Duration) -> String {
    let nanoseconds = duration.as_nanos();
    if nanoseconds < NANOS_PER_SECOND {
        milliseconds(duration)
    } else if nanoseconds < NANOS_PER_MINUTE {
        scaled(nanoseconds, NANOS_PER_SECOND, DISPLAY_SCALE, 9, "s")
    } else {
        scaled(nanoseconds, NANOS_PER_MINUTE, DISPLAY_SCALE, 9, "min")
    }
}
pub(super) fn milliseconds(duration: Duration) -> String {
    scaled(
        duration.as_nanos(),
        NANOS_PER_MILLISECOND,
        MILLISECOND_SCALE,
        6,
        "ms",
    )
}
#[expect(
    clippy::integer_division,
    reason = "duration units require exact integer quotient and remainder formatting"
)]
#[expect(
    clippy::integer_division_remainder_used,
    reason = "duration units require exact integer quotient and remainder formatting"
)]
fn scaled(
    nanoseconds: u128,
    nanoseconds_per_unit: u128,
    scale: u128,
    width: usize,
    suffix: &str,
) -> String {
    let whole = nanoseconds / nanoseconds_per_unit;
    let remainder = nanoseconds % nanoseconds_per_unit;
    let rounded_numerator = remainder * scale + nanoseconds_per_unit / 2;
    let fraction = rounded_numerator / nanoseconds_per_unit;
    let (rounded_whole, normalized_fraction) = if fraction == scale {
        (whole + 1, 0)
    } else {
        (whole, fraction)
    };
    if normalized_fraction == 0 {
        return format!("{rounded_whole}{suffix}");
    }
    let fraction_text = format!("{normalized_fraction:0width$}");
    let trimmed_fraction = fraction_text.trim_end_matches('0');
    format!("{rounded_whole}.{trimmed_fraction}{suffix}")
}
