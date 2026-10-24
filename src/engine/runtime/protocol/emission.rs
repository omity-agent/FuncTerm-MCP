use clap::ValueEnum;
use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, ValueEnum, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum OutputMode {
    Separated,
    Terminal,
}
impl OutputMode {
    pub(crate) const fn separate_output(self) -> bool {
        matches!(self, Self::Separated)
    }
}
#[derive(Clone, Debug, Deserialize, PartialEq, Eq, Serialize)]
pub(crate) enum CommandOutput {
    Separated { stdout: String, stderr: String },
    Terminal { output: String },
}
#[derive(Debug, Serialize, rmcp::schemars::JsonSchema)]
#[serde(untagged)]
pub(crate) enum OutputPresentation<'view> {
    Separated {
        stdout: &'view str,
        stderr: &'view str,
    },
    Terminal {
        output: &'view str,
    },
}
impl CommandOutput {
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow output buffers without cloning or ref bindings"
    )]
    pub(crate) const fn is_empty(&self) -> bool {
        match self {
            Self::Separated { stdout, stderr } => stdout.is_empty() && stderr.is_empty(),
            Self::Terminal { output } => output.is_empty(),
        }
    }
    #[expect(
        clippy::pattern_type_mismatch,
        reason = "borrow output buffers without cloning or ref bindings"
    )]
    pub(crate) fn presentation(&self) -> OutputPresentation<'_> {
        match self {
            Self::Separated { stdout, stderr } => OutputPresentation::Separated { stdout, stderr },
            Self::Terminal { output } => OutputPresentation::Terminal { output },
        }
    }
}
