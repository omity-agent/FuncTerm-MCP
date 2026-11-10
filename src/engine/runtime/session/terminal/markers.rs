use crate::contract::{
    TERMINAL_MARKER_CODE, TERMINAL_MARKER_END, TERMINAL_MARKER_NAME, TERMINAL_MARKER_START,
};
use anyhow::{Context as _, Result, bail};
use wezterm_escape_parser::{Action, OperatingSystemCommand};
pub(super) enum ProtocolEvent {
    Start(String),
    End(String),
    Prompt(String),
}
#[expect(
    clippy::pattern_type_mismatch,
    reason = "inspect borrowed escape actions without cloning OSC payloads"
)]
pub(super) fn parse(action: &Action) -> Result<Option<ProtocolEvent>> {
    let Action::OperatingSystemCommand(osc) = action else {
        return Ok(None);
    };
    let OperatingSystemCommand::Unspecified(parameters) = osc.as_ref() else {
        return Ok(None);
    };
    let Some(([code, name], payload)) = parameters.split_first_chunk::<2>() else {
        return Ok(None);
    };
    if code.as_slice() != TERMINAL_MARKER_CODE || name.as_slice() != TERMINAL_MARKER_NAME {
        return Ok(None);
    }
    if let Some(([phase, shell], cwd)) = payload.split_first_chunk::<2>()
        && phase == b"prompt"
        && matches!(shell.as_slice(), b"cmd" | b"bash" | b"zsh")
    {
        let path =
            String::from_utf8(cwd.join(b";".as_slice())).context("prompt cwd is not UTF-8")?;
        return Ok(Some(ProtocolEvent::Prompt(path)));
    }
    let [phase, raw_id] = payload else {
        bail!("terminal marker has invalid fields");
    };
    let id = str::from_utf8(raw_id)
        .context("terminal marker contains an invalid command id")?
        .to_owned();
    Ok(Some(if phase.as_slice() == TERMINAL_MARKER_START {
        ProtocolEvent::Start(id)
    } else if phase.as_slice() == TERMINAL_MARKER_END {
        ProtocolEvent::End(id)
    } else {
        bail!("terminal marker has unknown phase {phase:?}");
    }))
}
