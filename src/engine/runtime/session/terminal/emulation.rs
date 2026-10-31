use super::Terminal;
use alloc::sync::{Arc, Weak};
use core::sync::atomic::{AtomicUsize, Ordering};
use parking_lot::Mutex;
use portable_pty::PtySize;
use std::io::{self, Write};
use wezterm_term::{TerminalConfiguration, TerminalSize, color::ColorPalette};
#[derive(Debug, Default)]
struct EmulatorConfiguration {
    history: AtomicUsize,
}
impl TerminalConfiguration for EmulatorConfiguration {
    fn scrollback_size(&self) -> usize {
        self.history.load(Ordering::Relaxed)
    }
    fn color_palette(&self) -> ColorPalette {
        ColorPalette::default()
    }
}
pub(super) fn emulator(size: PtySize, writer: Box<dyn Write + Send>) -> wezterm_term::Terminal {
    let configuration = Arc::new(EmulatorConfiguration::default());
    let dimensions = TerminalSize {
        rows: usize::from(size.rows),
        cols: usize::from(size.cols),
        pixel_width: usize::from(size.pixel_width),
        pixel_height: usize::from(size.pixel_height),
        dpi: 0,
    };
    let model = wezterm_term::Terminal::new(
        dimensions,
        Arc::<EmulatorConfiguration>::clone(&configuration),
        env!("CARGO_PKG_NAME"),
        env!("CARGO_PKG_VERSION"),
        writer,
    );
    // WezTerm reserves history at construction. Enable retention after allocating the viewport.
    configuration.history.store(
        isize::MAX.cast_unsigned() - dimensions.rows,
        Ordering::Relaxed,
    );
    model
}
pub(super) struct PtyResponder {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    terminal: Weak<Terminal>,
}
impl PtyResponder {
    pub(super) const fn new(
        writer: Arc<Mutex<Box<dyn Write + Send>>>,
        terminal: Weak<Terminal>,
    ) -> Self {
        Self { writer, terminal }
    }
    fn report<T>(&self, result: io::Result<T>) -> io::Result<T> {
        if let Err(error) = result.as_ref() {
            let message = format!("failed to answer terminal host query: {error}");
            tracing::error!("{message}");
            if let Some(terminal) = self.terminal.upgrade() {
                terminal.reader_failed(&message);
            }
        }
        result
    }
}
impl Write for PtyResponder {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        tracing::debug!(bytes = ?buf, "writing terminal host query reply");
        let written = self.writer.lock().write_all(buf);
        self.report(written)?;
        Ok(buf.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        let flushed = self.writer.lock().flush();
        self.report(flushed)
    }
}
