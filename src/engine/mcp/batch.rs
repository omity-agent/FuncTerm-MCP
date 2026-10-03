use super::{collection, operations::Operation};
use crate::runtime::{client, config::Settings, protocol::wait_timeout_from_seconds};
use core::time::Duration;
use rmcp::model::CallToolResult;
use serde::Deserialize;
use std::{collections::HashSet, time::Instant};
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct ExecBatch<T> {
    #[schemars(length(min = 1))]
    pub(super) exec: Vec<T>,
}
#[derive(Debug, Deserialize, rmcp :: schemars :: JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct TimedBatch<T> {
    #[schemars(length(min = 1))]
    pub(super) exec: Vec<T>,
    #[schemars(range(min = 0))]
    pub(super) wait_timeout: f64,
}
pub(super) async fn run<T>(
    settings: &Settings,
    entries: Vec<T>,
    wait_timeout: Option<f64>,
) -> Result<CallToolResult, String>
where
    T: Operation,
{
    validate(&entries)?;
    let budget = wait_timeout
        .map(wait_timeout_from_seconds)
        .transpose()
        .map_err(super::error_text)?
        .unwrap_or(Duration::ZERO);
    client::ensure_daemon(settings)
        .await
        .map_err(super::error_text)?;
    let started = Instant::now();
    let operations = entries.into_iter().map(|entry| async move {
        let mut connection = client::DaemonClient::connect(settings)
            .await
            .map_err(super::error_text)?;
        let request = entry.request(settings, budget.saturating_sub(started.elapsed()))?;
        let payload = connection.call(request).await.map_err(super::error_text)?;
        T::output(payload)
    });
    let results = futures_util::future::join_all(operations).await;
    collection::combine(results)
}
fn validate<T>(entries: &[T]) -> Result<(), String>
where
    T: Operation,
{
    if entries.is_empty() {
        return Err(format!(
            "{} must contain at least one item",
            T::LIST_PARAMETER
        ));
    }
    let mut tab_ids = HashSet::new();
    for entry in entries {
        if let Some(tab_id) = entry.tab_id()
            && !tab_ids.insert(tab_id)
        {
            return Err(format!(
                "{} contains duplicate tab_id: {tab_id}",
                T::LIST_PARAMETER
            ));
        }
    }
    Ok(())
}
