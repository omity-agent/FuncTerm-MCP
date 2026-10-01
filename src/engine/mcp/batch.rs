use super::{collection, operations::Operation};
use crate::runtime::{client, protocol::wait_timeout_from_seconds};
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
    service_name: &str,
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
    let daemon_name = service_name.to_owned();
    tokio::task::spawn_blocking(move || client::ensure_daemon(&daemon_name))
        .await
        .map_err(super::error_text)?
        .map_err(super::error_text)?;
    let started = Instant::now();
    let workers: Vec<_> = entries
        .into_iter()
        .map(|entry| {
            let connection_name = service_name.to_owned();
            tokio::task::spawn_blocking(move || {
                let mut connection =
                    client::DaemonClient::connect(&connection_name).map_err(super::error_text)?;
                entry.execute(&mut connection, budget.saturating_sub(started.elapsed()))
            })
        })
        .collect();
    let mut results = Vec::with_capacity(workers.len());
    for worker in workers {
        results.push(match worker.await {
            Ok(result) => result,
            Err(error) => Err(format!("batch worker failed: {error}")),
        });
    }
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
