use super::emulation;
use anyhow::{Context as _, Result, ensure};
use portable_pty::PtySize;
use std::io::Write;
use wezterm_escape_parser::{
    Action, CSI,
    csi::{Edit, EraseInDisplay},
    esc::{Esc, EscCode},
};
use wezterm_term::StableRowIndex;
pub(super) struct CommandScreen {
    pub(super) model: wezterm_term::Terminal,
    start: Option<(StableRowIndex, usize)>,
}
impl CommandScreen {
    pub(super) fn new(size: PtySize, writer: Box<dyn Write + Send>) -> Self {
        Self {
            model: emulation::emulator(size, writer),
            start: None,
        }
    }
    pub(super) fn apply(&mut self, actions: Vec<Action>) {
        let reset = matches!(
            actions.last(),
            Some(Action::Esc(Esc::Code(EscCode::FullReset)))
        );
        let cleared = reset
            || matches!(
                actions.last(),
                Some(Action::CSI(CSI::Edit(Edit::EraseInDisplay(
                    EraseInDisplay::EraseDisplay
                ))))
            );
        self.model.perform_actions(actions);
        if cleared
            && !self.model.is_alt_screen_active()
            && let Some(start) = self.start.as_mut()
        {
            *start = (*start).min((self.model.screen().visible_row_to_stable_row(0), 0));
        }
        if self.start.is_none() {
            self.model.screen_mut().erase_scrollback();
        }
    }
    pub(super) fn begin(&mut self) -> Result<()> {
        ensure!(
            self.start.is_none(),
            "terminal output region is already active"
        );
        let cursor = self.model.cursor_pos();
        self.start = Some((
            self.model.screen().visible_row_to_stable_row(cursor.y),
            cursor.x,
        ));
        Ok(())
    }
    pub(super) fn contents(&self) -> Result<String> {
        let initial = self.start.context("terminal output region is not active")?;
        let start = if self.model.is_alt_screen_active() {
            (self.model.screen().phys_to_stable_row_index(0), 0)
        } else {
            initial.max((self.model.screen().phys_to_stable_row_index(0), 0))
        };
        Ok(self.text(start, true))
    }
    pub(super) fn finish(&mut self) -> Result<String> {
        let output = self.contents()?;
        self.start = None;
        self.model.screen_mut().erase_scrollback();
        Ok(output)
    }
    pub(super) fn visible_contents(&self) -> String {
        self.text((self.model.screen().visible_row_to_stable_row(0), 0), false)
    }
    fn text(&self, start: (StableRowIndex, usize), join_wrapped: bool) -> String {
        let screen = self.model.screen();
        let cursor = self.model.cursor_pos();
        let cursor_row = screen.visible_row_to_stable_row(cursor.y);
        let end = screen.phys_to_stable_row_index(screen.scrollback_rows());
        let mut output = String::new();
        let mut first = true;
        let mut wrapped = false;
        let mut populated = 0;
        screen.for_each_logical_line_in_stable_range(start.0..end, |range, lines| {
            for (row, line) in range.zip(lines) {
                if row >= start.0 {
                    if !(first || join_wrapped && wrapped) {
                        output.push('\n');
                    }
                    let from = if first { start.1 } else { 0 };
                    first = false;
                    let mut text = line.columns_as_str(from..screen.physical_cols);
                    wrapped = line.last_cell_was_wrapped();
                    let beyond_cursor = row > cursor_row || row == cursor_row && cursor.x == 0;
                    let retained = if beyond_cursor && text.trim_end_matches(' ').is_empty() {
                        0
                    } else if join_wrapped && wrapped {
                        text.len()
                    } else {
                        let cursor_text = if row == cursor_row {
                            line.columns_as_str(from..cursor.x)
                        } else {
                            String::new()
                        };
                        text.trim_end_matches(' ').len().max(cursor_text.len())
                    };
                    if retained > 0 {
                        text.truncate(retained);
                        output.push_str(&text);
                        populated = output.len();
                    }
                }
            }
            true
        });
        output.truncate(populated);
        output
    }
}
