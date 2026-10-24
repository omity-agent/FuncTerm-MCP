const ETX: u8 = 0x03;
#[cfg(not(windows))]
const ETX_BYTES: &[u8] = &[ETX];
pub(super) struct InputBatch<'input> {
    bytes: &'input [u8],
}
#[derive(Clone, Copy)]
pub(super) struct InputDelivery {
    interrupted: bool,
}
impl<'input> InputBatch<'input> {
    pub(super) const fn from_bytes(bytes: &'input [u8]) -> Self {
        Self { bytes }
    }
    pub(super) fn segments(&self) -> impl Iterator<Item = &'input [u8]> {
        self.bytes
            .split_inclusive(|byte| *byte == ETX)
            .flat_map(|chunk| {
                chunk
                    .strip_suffix(&[ETX])
                    .map_or([chunk, &[]], |data| [data, interrupt_bytes()])
            })
            .filter(|segment| !segment.is_empty())
    }
    pub(super) fn delivery(&self) -> InputDelivery {
        InputDelivery {
            interrupted: self.bytes.contains(&ETX),
        }
    }
}
impl InputDelivery {
    pub(super) const fn interrupted(self) -> bool {
        self.interrupted
    }
}
#[cfg(not(windows))]
const fn interrupt_bytes() -> &'static [u8] {
    ETX_BYTES
}
#[cfg(windows)]
const fn interrupt_bytes() -> &'static [u8] {
    concat!(
        "\x1b[17;29;0;1;8;1_",
        "\x1b[67;46;3;1;8;1_",
        "\x1b[67;46;3;0;8;1_",
        "\x1b[17;29;0;0;0;1_",
    )
    .as_bytes()
}
