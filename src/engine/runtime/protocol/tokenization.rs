use super::{CommandOutput, OutputPresentation, ViewResult};
use anyhow::{Context as _, Result, ensure};
use core::num::NonZeroUsize;
pub(crate) struct OutputTokenizer {
    tokenizer: toktok::Tokenizer,
}
impl OutputTokenizer {
    pub(crate) fn new(encoding: &str) -> Result<Self> {
        Ok(Self {
            tokenizer: toktok::Tokenizer::builtin(encoding)
                .with_context(|| format!("failed to initialize output tokenizer {encoding}"))?,
        })
    }
    pub(crate) fn count(&self, command_output: &CommandOutput) -> Result<usize> {
        match command_output.presentation() {
            OutputPresentation::Terminal { output } => Ok(self.tokenizer.count(output.as_bytes())),
            OutputPresentation::Separated { stdout, stderr } => self
                .tokenizer
                .count(stdout.as_bytes())
                .checked_add(self.tokenizer.count(stderr.as_bytes()))
                .context("combined output token count exceeds the supported range"),
        }
    }
    pub(crate) fn start_at(
        &self,
        view: ViewResult,
        start_token: NonZeroUsize,
    ) -> Result<ViewResult> {
        match view {
            ViewResult::Tab {
                shell,
                mut screen,
                note,
            } => {
                self.skip_tokens(&mut screen, start_token);
                Ok(ViewResult::Tab {
                    shell,
                    screen,
                    note,
                })
            }
            ViewResult::Command {
                shell,
                mut command,
                note,
            } => {
                command.output = match command.output {
                    CommandOutput::Terminal { mut output } => {
                        self.skip_tokens(&mut output, start_token);
                        CommandOutput::Terminal { output }
                    }
                    separated @ CommandOutput::Separated {
                        stdout: _,
                        stderr: _,
                    } => {
                        ensure!(
                            start_token == NonZeroUsize::MIN,
                            "start_token must be 1 for separated stdout/stderr output"
                        );
                        separated
                    }
                };
                Ok(ViewResult::Command {
                    shell,
                    command,
                    note,
                })
            }
        }
    }
    fn skip_tokens(&self, text: &mut String, start_token: NonZeroUsize) {
        let prefix = self
            .tokenizer
            .truncate(text.as_bytes(), start_token.get() - 1);
        drop(text.drain(..prefix.bytes));
    }
}
