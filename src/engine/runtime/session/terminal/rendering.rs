use super::emulation;
use anyhow::{Context as _, Result, ensure};
use bytestring::ByteString;
use portable_pty::PtySize;
use std::io::Write;
use wezterm_escape_parser::{
    Action, CSI,
    csi::{Edit, EraseInDisplay},
    esc::{Esc, EscCode},
};
use wezterm_term::{CellRef, Line, StableRowIndex};
pub(super) struct CommandScreen {
    pub(super) model: wezterm_term::Terminal,
    start: Option<(StableRowIndex, usize)>,
    cached_output: Option<ByteString>,
}
impl CommandScreen {
    pub(super) fn new(size: PtySize, writer: Box<dyn Write + Send>) -> Self {
        Self {
            model: emulation::emulator(size, writer),
            start: None,
            cached_output: None,
        }
    }
    pub(super) fn apply(&mut self, actions: Vec<Action>) {
        self.cached_output = None;
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
        self.cached_output = None;
        self.start = Some((
            self.model.screen().visible_row_to_stable_row(cursor.y),
            cursor.x,
        ));
        Ok(())
    }
    pub(super) fn contents(&mut self) -> Result<ByteString> {
        let initial = self.start.context("terminal output region is not active")?;
        if let Some(output) = self.cached_output.as_ref() {
            return Ok(output.clone());
        }
        let start = if self.model.is_alt_screen_active() {
            (self.model.screen().phys_to_stable_row_index(0), 0)
        } else {
            initial.max((self.model.screen().phys_to_stable_row_index(0), 0))
        };
        let output: ByteString = self.text(start, true).into();
        self.cached_output = Some(output.clone());
        Ok(output)
    }
    pub(super) fn finish(&mut self) -> Result<ByteString> {
        let output = self.contents()?;
        self.start = None;
        self.cached_output = None;
        self.model.screen_mut().erase_scrollback();
        Ok(output)
    }
    pub(super) fn visible_contents(&mut self) -> String {
        self.text((self.model.screen().visible_row_to_stable_row(0), 0), false)
    }
    fn text(&mut self, start: (StableRowIndex, usize), join_wrapped: bool) -> String {
        let cursor = self.model.cursor_pos();
        let screen = self.model.screen_mut();
        let columns = screen.physical_cols;
        let cursor_row = screen.phys_row(cursor.y);
        let end = screen.phys_to_stable_row_index(screen.scrollback_rows());
        let physical = screen.stable_range(&(start.0..end));
        let first = physical.start;
        let mut output = String::new();
        let mut wrapped = false;
        let mut populated = 0;
        for row in physical {
            if row != first && !wrapped {
                output.push('\n');
            }
            let line = screen.line_mut(row);
            let from = if row == first { start.1 } else { 0 };
            let mut text = line.columns_as_str(from..columns);
            wrapped = join_wrapped && line_wraps(line);
            let beyond_cursor = row > cursor_row || row == cursor_row && cursor.x == 0;
            let trimmed = text.trim_end_matches(' ').len();
            let retained = if beyond_cursor && trimmed == 0 {
                0
            } else if wrapped {
                text.len()
            } else if row == cursor_row {
                trimmed.max(line.columns_as_str(from..cursor.x).len())
            } else {
                trimmed
            };
            if retained > 0 {
                text.truncate(retained);
                output.push_str(&text);
                populated = output.len();
            }
        }
        output.truncate(populated);
        output
    }
}
fn line_wraps(line: &mut Line) -> bool {
    let Some(cell) = line.visible_cells().last() else {
        return false;
    };
    cell.attrs().wrapped()
        || matches!(
            cell,
            CellRef::CellRef {
                cell_index: _,
                cell: _
            }
        ) && cell.width() > 1
            && line
                .cells_mut_for_attr_changes_only()
                .last()
                .is_some_and(|tail| tail.attrs().wrapped())
}
