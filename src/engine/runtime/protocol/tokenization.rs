use super::{CommandOutput, ViewResult};
use anyhow::{Context as _, Result, ensure};
use bytestring::ByteString;
use core::num::NonZeroUsize;
pub(crate) struct OutputTokenizer {
    tokenizer: toktok::Tokenizer,
}
#[derive(Default)]
pub(crate) struct OutputTokens {
    text: ByteString,
    tokens: usize,
}
impl OutputTokens {
    pub(crate) fn count(&mut self, tokenizer: &OutputTokenizer, text: &ByteString) -> usize {
        if self.text != *text {
            self.tokens = tokenizer.tokenizer.count(text.as_bytes());
            self.text = text.clone();
        }
        self.tokens
    }
}
impl OutputTokenizer {
    pub(crate) fn new(encoding: &str) -> Result<Self> {
        Ok(Self {
            tokenizer: toktok::Tokenizer::builtin(encoding)
                .with_context(|| format!("failed to initialize output tokenizer {encoding}"))?,
        })
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
        if start_token == NonZeroUsize::MIN {
            return Ok(text.clone());
        }
        let prefix = self
            .tokenizer
            .truncate(text.as_bytes(), start_token.get() - 1);
        let suffix = text
            .get(prefix.bytes..)
            .context("output tokenizer returned an invalid UTF-8 boundary")?;
        Ok(text.slice_ref(suffix))
    }
}
