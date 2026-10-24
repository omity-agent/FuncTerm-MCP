use crate::contract::{
    TERMINAL_MARKER_CODE, TERMINAL_MARKER_END, TERMINAL_MARKER_NAME, TERMINAL_MARKER_START,
};
use vte::{Params, Perform};
pub(super) struct OutputParser {
    parser: vte::Parser,
    detector: Detector,
}
impl OutputParser {
    pub(super) fn new() -> Self {
        Self {
            parser: vte::Parser::new(),
            detector: Detector::default(),
        }
    }
    pub(super) fn advance(&mut self, bytes: &[u8]) -> (usize, Option<OutputEvent>) {
        self.detector.event = None;
        let consumed = self
            .parser
            .advance_until_terminated(&mut self.detector, bytes);
        (consumed, self.detector.event.take())
    }
}
pub(super) enum OutputEvent {
    Capture(ProtocolEvent),
    ScreenCheckpoint,
}
pub(super) enum ProtocolEvent {
    Start(String),
    End(String),
    WindowTitleAssigned,
    Invalid(String),
}
#[derive(Default)]
struct Detector {
    event: Option<OutputEvent>,
}
impl Perform for Detector {
    fn csi_dispatch(
        &mut self,
        _params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        _action: char,
    ) {
        self.event = Some(OutputEvent::ScreenCheckpoint);
    }
    fn esc_dispatch(&mut self, _intermediates: &[u8], _ignore: bool, _byte: u8) {
        self.event = Some(OutputEvent::ScreenCheckpoint);
    }
    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        if matches ! (params . first () , Some (code) if matches ! (* code , b"0" | b"2")) {
            self.event = Some(OutputEvent::Capture(ProtocolEvent::WindowTitleAssigned));
            return;
        }
        let &[code, name, phase, raw_id] = params else {
            return;
        };
        if code != TERMINAL_MARKER_CODE || name != TERMINAL_MARKER_NAME {
            return;
        }
        let id = match str::from_utf8(raw_id) {
            Ok(value) => value.to_owned(),
            Err(error) => {
                self.event = Some(OutputEvent::Capture(ProtocolEvent::Invalid(format!(
                    "terminal marker contains an invalid command id: {error}"
                ))));
                return;
            }
        };
        self.event = Some(OutputEvent::Capture(if phase == TERMINAL_MARKER_START {
            ProtocolEvent::Start(id)
        } else if phase == TERMINAL_MARKER_END {
            ProtocolEvent::End(id)
        } else {
            ProtocolEvent::Invalid(format!("terminal marker has unknown phase {phase:?}"))
        }));
    }
    fn terminated(&self) -> bool {
        self.event.is_some()
    }
}
