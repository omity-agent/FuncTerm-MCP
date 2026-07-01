use anyhow::{Context as _, Result};
use iceoryx2::port::listener::Listener;
use iceoryx2::port::notifier::Notifier;
use iceoryx2::prelude::*;
const EVENT_CAPACITY: usize = 128;
pub(crate) type IpcListener = Listener<ipc_threadsafe::Service>;
pub(crate) type IpcNotifier = Notifier<ipc_threadsafe::Service>;
pub(crate) fn request_listener(
    node: &Node<ipc_threadsafe::Service>,
    service_name: &str,
) -> Result<IpcListener> {
    event_factory(node, &request_event_name(service_name))?
        .listener_builder()
        .create()
        .context("failed to create iceoryx2 request event listener")
}
pub(crate) fn request_notifier(
    node: &Node<ipc_threadsafe::Service>,
    service_name: &str,
) -> Result<IpcNotifier> {
    event_factory(node, &request_event_name(service_name))?
        .notifier_builder()
        .create()
        .context("failed to create iceoryx2 request event notifier")
}
pub(crate) fn response_listener(
    node: &Node<ipc_threadsafe::Service>,
    service_name: &str,
) -> Result<IpcListener> {
    event_factory(node, &response_event_name(service_name))?
        .listener_builder()
        .create()
        .context("failed to create iceoryx2 response event listener")
}
pub(crate) fn response_notifier(
    node: &Node<ipc_threadsafe::Service>,
    service_name: &str,
) -> Result<IpcNotifier> {
    event_factory(node, &response_event_name(service_name))?
        .notifier_builder()
        .create()
        .context("failed to create iceoryx2 response event notifier")
}
pub(crate) fn ready_listener(
    node: &Node<ipc_threadsafe::Service>,
    service_name: &str,
) -> Result<IpcListener> {
    event_factory(node, &ready_event_name(service_name))?
        .listener_builder()
        .create()
        .context("failed to create iceoryx2 ready event listener")
}
pub(crate) fn ready_notifier(
    node: &Node<ipc_threadsafe::Service>,
    service_name: &str,
) -> Result<IpcNotifier> {
    event_factory(node, &ready_event_name(service_name))?
        .notifier_builder()
        .create()
        .context("failed to create iceoryx2 ready event notifier")
}
fn event_factory(
    node: &Node<ipc_threadsafe::Service>,
    name: &str,
) -> Result<iceoryx2::service::port_factory::event::PortFactory<ipc_threadsafe::Service>> {
    node.service_builder(&name.try_into()?)
        .event()
        .max_listeners(EVENT_CAPACITY)
        .max_notifiers(EVENT_CAPACITY)
        .open_or_create()
        .with_context(|| format!("failed to open iceoryx2 event service {name}"))
}
fn request_event_name(service_name: &str) -> String {
    event_name(service_name, "request")
}
fn response_event_name(service_name: &str) -> String {
    event_name(service_name, "response")
}
fn ready_event_name(service_name: &str) -> String {
    event_name(service_name, "ready")
}
fn event_name(service_name: &str, suffix: &str) -> String {
    format!("{service_name}/{suffix}_event")
}
