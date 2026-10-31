use alloc::sync::Arc;
use anyhow::{Context as _, Result, ensure};
use bytestring::ByteString;
use itertools::Itertools as _;
use parking_lot::Mutex;
use tastty_core::{
    AbsolutePosition, Parser, PlainOptions, Position, ScreenEvent, TerminalMode, TerminalSize,
    screen::LogicalLineOptions,
};
pub(super) struct CommandScreen {
    pub(super) parser: Parser,
    start: Option<AbsolutePosition>,
}
pub(super) enum RenderedOutput {
    Pending(Arc<Mutex<CommandScreen>>),
    Active(Arc<Mutex<CommandScreen>>),
    Frozen(ByteString),
}
impl CommandScreen {
    pub(super) fn new(size: TerminalSize) -> Self {
        Self {
            parser: Parser::new(size, usize::MAX),
            start: None,
        }
    }
    fn begin(&mut self) -> Result<()> {
        ensure!(
            self.start.is_none(),
            "terminal output region is already active"
        );
        let screen = self.parser.screen();
        let cursor = screen.cursor();
        let mut start = screen
            .visible_to_absolute(Position::new(cursor.row, 0))
            .context("command start cursor is outside the terminal")?;
        start.col = cursor.col;
        self.start = Some(start);
        Ok(())
    }
    pub(super) fn observe(&mut self, event: &ScreenEvent) -> Result<()> {
        if matches!(event, ScreenEvent::ScreenCleared)
            && !self.parser.screen().mode(TerminalMode::AlternateScreen)
            && let Some(start) = self.start.as_mut()
        {
            let top = self
                .parser
                .screen()
                .visible_to_absolute(Position::new(0, 0))
                .context("cleared terminal has no first row")?;
            *start = (*start).min(top);
        }
        Ok(())
    }
    pub(super) fn checkpoint(&mut self) {
        if self.start.is_none() && self.parser.screen().scrollback_available() > 0 {
            self.parser.process(b"\x1b[3J");
        }
    }
    fn contents(&mut self) -> Result<String> {
        let initial = self.start.context("terminal output region is not active")?;
        let screen = self.parser.screen_mut();
        if screen.mode(TerminalMode::AlternateScreen) {
            let mut options = PlainOptions::default();
            options.join_wrapped = true;
            options.include_scrollback = true;
            return Ok(screen.contents_plain(options));
        }
        let oldest = AbsolutePosition::new(screen.scrollback_origin_row(), 0);
        let start = initial.max(oldest);
        let options = LogicalLineOptions {
            join_wrapped: true,
            include_scrollback: true,
            trim_trailing_blanks: true,
        };
        let mut output = String::new();
        for (index, line) in screen
            .logical_lines(options)
            .filter(|line| line.end.row >= start.row)
            .enumerate()
        {
            if index > 0 {
                output.push('\n');
            }
            for (_, cells) in &line.cells().chunk_by(|cell| cell.0.row) {
                let mut physical_row = String::new();
                let mut populated = 0;
                for (position, cell) in cells {
                    if position >= start {
                        if cell.contents().is_empty() {
                            physical_row.push(' ');
                        } else {
                            physical_row.push_str(cell.contents());
                        }
                        if cell.has_contents() {
                            populated = physical_row.len();
                        }
                    }
                }
                physical_row.truncate(populated);
                output.push_str(&physical_row);
            }
        }
        Ok(output)
    }
    fn finish(&mut self) -> Result<String> {
        let output = self.contents()?;
        self.start = None;
        Ok(output)
    }
}
impl RenderedOutput {
    pub(super) const fn new(screen: Arc<Mutex<CommandScreen>>) -> Self {
        Self::Pending(screen)
    }
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow the pending screen while advancing the capture state"
    )]
    pub(super) fn start(&mut self) -> Result<()> {
        let Self::Pending(screen) = self else {
            anyhow::bail!("terminal output capture is not pending");
        };
        screen.lock().begin()?;
        *self = Self::Active(Arc::clone(screen));
        Ok(())
    }
    pub(super) fn contents(&self) -> Result<ByteString> {
        #[expect(
            clippy::pattern_type_mismatch,
            reason = "borrow the live screen or frozen output without moving it"
        )]
        match self {
            Self::Pending(_) => Ok(ByteString::new()),
            Self::Active(screen) => Ok(screen.lock().contents()?.into()),
            Self::Frozen(output) => Ok(output.clone()),
        }
    }
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow the active screen before replacing it with its frozen output"
    )]
    pub(super) fn finish(&mut self) -> Result<()> {
        let output = match self {
            Self::Pending(_) => String::new(),
            Self::Active(screen) => screen.lock().finish()?,
            Self::Frozen(_) => return Ok(()),
        };
        *self = Self::Frozen(output.into());
        Ok(())
    }
}
