use super::{CommandOutput, OutputPresentation, ViewResult};
use anyhow::{Context as _, Result, ensure};
use bytestring::ByteString;
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
                screen,
                note,
            } => Ok(ViewResult::Tab {
                shell,
                screen: self.skip_tokens(&screen, start_token)?,
                note,
            }),
            ViewResult::Command {
                shell,
                mut command,
                note,
            } => {
                command.output = match command.output {
                    CommandOutput::Terminal { output } => CommandOutput::Terminal {
                        output: self.skip_tokens(&output, start_token)?,
                    },
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
    fn skip_tokens(&self, text: &ByteString, start_token: NonZeroUsize) -> Result<ByteString> {
        let prefix = self
            .tokenizer
            .truncate(text.as_bytes(), start_token.get() - 1);
        let suffix = text
            .get(prefix.bytes..)
            .context("output tokenizer returned an invalid UTF-8 boundary")?;
        Ok(text.slice_ref(suffix))
    }
}
