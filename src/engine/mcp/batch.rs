use super::{collection, invocation::Backend, operations::Operation};
use crate::runtime::{config::Settings, protocol::wait_timeout_from_seconds};
use anyhow::{Context as _, Result, ensure};
use core::str::FromStr;
use core::time::Duration;
use rmcp::model::CallToolResult;
use serde::Deserialize;
use std::{collections::HashSet, time::Instant};
#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(try_from = "f64")]
pub(crate) struct WaitTimeoutMultiplier(f64);
impl TryFrom<f64> for WaitTimeoutMultiplier {
    type Error = anyhow::Error;
    fn try_from(value: f64) -> Result<Self> {
        ensure!(
            value.is_finite() && value > 0.0_f64,
            "wait timeout multiplier must be finite and greater than zero"
        );
        Ok(Self(value))
    }
}
impl FromStr for WaitTimeoutMultiplier {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self> {
        Self::try_from(value.parse::<f64>()?)
    }
}
impl WaitTimeoutMultiplier {
    fn scale(self, seconds: f64) -> Result<Duration> {
        wait_timeout_from_seconds(seconds)?;
        wait_timeout_from_seconds(seconds * self.0)
            .context("scaled wait timeout exceeds the supported duration range")
    }
}
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
    backend: &Backend,
    entries: Vec<T>,
    wait_timeout: Option<f64>,
) -> Result<CallToolResult, String>
where
    T: Operation,
{
    validate(&entries)?;
    let budget = wait_timeout
        .map(|seconds| settings.mcp.wait_timeout_multiplier.scale(seconds))
        .transpose()
        .map_err(super::error_text)?
        .unwrap_or(Duration::ZERO);
    let prepared = backend.prepare(settings).await.map_err(super::error_text)?;
    let execution = &prepared;
    let started = Instant::now();
    let operations = entries.into_iter().map(|entry| async move {
        let request = entry.request(settings, budget.saturating_sub(started.elapsed()))?;
        let payload = execution
            .execute(settings, request)
            .await
            .map_err(super::error_text)?;
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
