use crate::runtime::protocol::{CommandOutput, OutputTokenizer, OutputTokens};
use crate::runtime::session::records::CommandRecord;
use anyhow::{Context as _, Result, bail, ensure};
use bytestring::ByteString;
use encoding_rs::{Decoder, DecoderResult};
use std::fs::File;
use std::io::{Read as _, Seek as _, SeekFrom};
use std::path::Path;
#[derive(Default)]
pub(super) struct SeparatedOutput {
    stdout: StreamOutput,
    stderr: StreamOutput,
    stdout_tokens: OutputTokens,
    stderr_tokens: OutputTokens,
}
struct StreamOutput {
    offset: u64,
    decoder: Decoder,
    decoded: String,
    visible: ByteString,
    finished: bool,
    failure: Option<String>,
}
impl Default for StreamOutput {
    fn default() -> Self {
        Self {
            offset: 0,
            decoder: encoding_rs::UTF_8.new_decoder(),
            decoded: String::new(),
            visible: ByteString::new(),
            finished: false,
            failure: None,
        }
    }
}
impl SeparatedOutput {
    pub(super) fn read(
        &mut self,
        record: &CommandRecord,
        tokenizer: &OutputTokenizer,
        finished: bool,
    ) -> Result<(CommandOutput, usize)> {
        self.stdout
            .refresh(&record.stdout, finished)
            .context("failed to read captured stdout")?;
        self.stderr
            .refresh(&record.stderr, finished)
            .context("failed to read captured stderr")?;
        let output = CommandOutput::Separated {
            stdout: self.stdout.visible.clone(),
            stderr: self.stderr.visible.clone(),
        };
        let tokens = self
            .stdout_tokens
            .count(tokenizer, &self.stdout.visible)
            .checked_add(self.stderr_tokens.count(tokenizer, &self.stderr.visible))
            .context("combined output token count exceeds the supported range")?;
        Ok((output, tokens))
    }
}
impl StreamOutput {
    fn refresh(&mut self, path: &Path, finished: bool) -> Result<()> {
        if let Some(message) = self.failure.as_deref() {
            bail!("{message}");
        }
        if self.finished {
            return Ok(());
        }
        let bytes = self.read_appended(path)?;
        if bytes.is_empty() && !finished {
            return Ok(());
        }
        let previous_len = self.decoded.len();
        if let Err(error) = self.decode(&bytes, finished) {
            self.failure = Some(format!("{error:#}"));
            return Err(error);
        }
        self.finished = finished;
        if self.decoded.len() == previous_len {
            return Ok(());
        }
        let text = fast_strip_ansi::strip_ansi_string(&self.decoded);
        if text.as_ref() == &*self.visible {
            return Ok(());
        }
        self.visible = text.into_owned().into();
        Ok(())
    }
    fn read_appended(&mut self, path: &Path) -> Result<Vec<u8>> {
        let mut file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound && self.offset == 0 => {
                return Ok(Vec::new());
            }
            Err(error) => return Err(error.into()),
        };
        let available = file
            .metadata()?
            .len()
            .checked_sub(self.offset)
            .context("captured output was truncated")?;
        if available == 0 {
            return Ok(Vec::new());
        }
        file.seek(SeekFrom::Start(self.offset))?;
        let mut bytes = Vec::new();
        file.take(available).read_to_end(&mut bytes)?;
        let consumed = u64::try_from(bytes.len())?;
        ensure!(
            consumed == available,
            "captured output changed while being read"
        );
        self.offset = self
            .offset
            .checked_add(consumed)
            .context("captured output position overflowed")?;
        Ok(bytes)
    }
    fn decode(&mut self, bytes: &[u8], finished: bool) -> Result<()> {
        let capacity = self
            .decoder
            .max_utf8_buffer_length_without_replacement(bytes.len())
            .context("decoded output exceeds the supported size")?;
        self.decoded
            .try_reserve(capacity)
            .context("failed to reserve decoded output storage")?;
        let (result, consumed) =
            self.decoder
                .decode_to_string_without_replacement(bytes, &mut self.decoded, finished);
        match result {
            DecoderResult::InputEmpty => ensure!(
                consumed == bytes.len(),
                "output decoder did not consume the captured bytes"
            ),
            DecoderResult::OutputFull => bail!("output decoder exceeded its reserved storage"),
            DecoderResult::Malformed(_, _) => {
                bail!("text is not valid {}", self.decoder.encoding().name());
            }
        }
        Ok(())
    }
}
