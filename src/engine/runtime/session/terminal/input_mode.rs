use anyhow::{Context as _, Result};
use vte::{Params, Perform};
pub(super) struct InputModeTracker {
    parser: vte::Parser,
    detector: ModeDetector,
    win32_input: bool,
}
impl InputModeTracker {
    pub(super) fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            detector: ModeDetector::default(),
            win32_input: false,
        }
    }
    pub(super) fn process_segments(
        &mut self,
        bytes: &[u8],
        mut process: impl FnMut(&[u8], bool),
    ) -> Result<()> {
        let mut segment_start = 0_usize;
        let mut segment_end = 0_usize;
        for byte in bytes {
            self.detector.transition = None;
            self.parser
                .advance(&mut self.detector, core::slice::from_ref(byte));
            segment_end = segment_end
                .checked_add(1)
                .context("terminal input mode offset overflow")?;
            let Some(enabled) = self.detector.transition else {
                continue;
            };
            let segment = bytes
                .get(segment_start..segment_end)
                .context("terminal input mode segment exceeds output")?;
            process(segment, self.win32_input);
            self.win32_input = enabled;
            segment_start = segment_end;
        }
        let tail = bytes
            .get(segment_start..)
            .context("terminal input mode tail exceeds output")?;
        process(tail, self.win32_input);
        Ok(())
    }
}
#[derive(Default)]
struct ModeDetector {
    transition: Option<bool>,
}
impl Perform for ModeDetector {
    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], ignore: bool, action: char) {
        if ignore || intermediates != b"?" || !matches!(action, 'h' | 'l') {
            return;
        }
        if params.iter().any(|param| param == [9001]) {
            self.transition = Some(action == 'h');
        }
    }
}
