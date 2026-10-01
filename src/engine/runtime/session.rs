mod keyboard;
mod manager;
pub(crate) mod records;
pub(crate) mod temp;
mod terminal;
pub(crate) use manager::Manager;
mod observation;
pub(crate) use observation::wait_for_path;
