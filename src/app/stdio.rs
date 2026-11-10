use crate::engine::{mcp::WaitTimeoutMultiplier, privileges::PrivilegeMode};
use crate::runtime::{config, protocol::OutputMode};
use anyhow::Result;
#[derive(Default, clap::Args)]
pub(super) struct Options {
    #[arg(
        long,
        help = "Disable Shell Profile loading for all Tabs created by this MCP server"
    )]
    no_profile: bool,
    #[arg(
        long,
        value_enum,
        help = "Fix New Tab permissions and hide its elevated input"
    )]
    privilege_mode: Option<PrivilegeMode>,
    #[arg(
        long,
        value_enum,
        help = "Fix command output handling and hide its separate_output input"
    )]
    output_mode: Option<OutputMode>,
    #[arg(
        long,
        allow_negative_numbers = true,
        help = "Minimum accepted MCP tool wait_timeout in seconds, before applying the multiplier"
    )]
    wait_timeout_min: Option<f64>,
    #[arg(
        long,
        allow_negative_numbers = true,
        help = "Maximum accepted MCP tool wait_timeout in seconds, before applying the multiplier"
    )]
    wait_timeout_max: Option<f64>,
    #[arg(
        long,
        help = "Multiply MCP tool wait_timeout values by this positive factor"
    )]
    wait_timeout_multiplier: Option<WaitTimeoutMultiplier>,
}
impl Options {
    pub(super) async fn run(self) -> Result<()> {
        let mut settings = config::load()?;
        settings.shell_load_profile &= !self.no_profile;
        let timing = &mut settings.mcp.wait_timeout;
        if let Some(minimum) = self.wait_timeout_min {
            timing.minimum = minimum;
        }
        if let Some(maximum) = self.wait_timeout_max {
            timing.maximum = Some(maximum);
        }
        if let Some(multiplier) = self.wait_timeout_multiplier {
            timing.multiplier = multiplier;
        }
        timing.validate()?;
        crate::mcp::run(settings, self.privilege_mode, self.output_mode).await
    }
}
