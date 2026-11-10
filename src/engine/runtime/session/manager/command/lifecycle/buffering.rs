use crate::runtime::protocol::{CommandOutput, OutputTokenizer, OutputTokens};
use crate::runtime::session::records::CommandRecord;
use anstream::adapter::StripBytes;
use anyhow::{Context as _, Result, bail, ensure};
use bytestring::ByteString;
use encoding_rs::{CoderResult, Decoder};
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
    stripper: StripBytes,
    visible: ByteString,
    finished: bool,
    failure: Option<String>,
}
impl Default for StreamOutput {
    fn default() -> Self {
        Self {
            offset: 0,
            decoder: encoding_rs::UTF_8.new_decoder(),
            stripper: StripBytes::default(),
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
        let decoded = self.decode(&bytes, finished).inspect_err(|error| {
            self.failure = Some(format!("{error:#}"));
        })?;
        self.finished = finished;
        let mut fragments = self.stripper.strip_next(decoded.as_bytes()).peekable();
        if fragments.peek().is_some() {
            let mut visible = self.visible.as_bytes().to_vec();
            for fragment in fragments {
                visible.extend_from_slice(fragment);
            }
            self.visible = String::from_utf8(visible)
                .context("ANSI stripping produced invalid UTF-8")?
                .into();
        }
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
    fn decode(&mut self, bytes: &[u8], finished: bool) -> Result<String> {
        let capacity = self
            .decoder
            .max_utf8_buffer_length(bytes.len())
            .context("decoded output exceeds the supported size")?;
        let mut decoded = String::new();
        decoded
            .try_reserve(capacity)
            .context("failed to reserve decoded output storage")?;
        let (result, consumed, _replaced) =
            self.decoder.decode_to_string(bytes, &mut decoded, finished);
        match result {
            CoderResult::InputEmpty => ensure!(
                consumed == bytes.len(),
                "output decoder did not consume the captured bytes"
            ),
            CoderResult::OutputFull => bail!("output decoder exceeded its reserved storage"),
        }
        Ok(decoded)
    }
}
