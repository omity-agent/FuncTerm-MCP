pub(crate) mod shared;
use anyhow::Result;
use iceoryx2::config::Config;
pub(crate) fn config() -> Result<Config> {
    shared::config()
}
