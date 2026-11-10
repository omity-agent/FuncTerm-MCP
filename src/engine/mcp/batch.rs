use super::{collection, invocation::Backend, operations::Operation};
use crate::runtime::{config::Settings, protocol::wait_timeout_from_seconds};
use anyhow::{Context as _, Result, ensure};
use core::str::FromStr;
use core::time::Duration;
use rmcp::model::{CallToolResult, JsonObject};
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
#[derive(Clone, Copy, Debug, Deserialize)]
pub(crate) struct WaitTimeoutSettings {
    #[serde(rename = "wait_timeout_min")]
    pub(crate) minimum: f64,
    #[serde(rename = "wait_timeout_max")]
    pub(crate) maximum: Option<f64>,
    #[serde(rename = "wait_timeout_multiplier")]
    pub(crate) multiplier: WaitTimeoutMultiplier,
}
impl WaitTimeoutSettings {
    pub(crate) fn validate(self) -> Result<()> {
        wait_timeout_from_seconds(self.minimum).context("invalid mcp.wait_timeout_min")?;
        if let Some(maximum) = self.maximum {
            wait_timeout_from_seconds(maximum).context("invalid mcp.wait_timeout_max")?;
            ensure!(
                maximum >= self.minimum,
                "mcp.wait_timeout_max must be greater than or equal to mcp.wait_timeout_min"
            );
        }
        Ok(())
    }
    pub(super) fn apply_schema(self, schema: &mut JsonObject) {
        schema.insert("minimum".to_owned(), self.minimum.into());
        if let Some(maximum) = self.maximum {
            schema.insert("maximum".to_owned(), maximum.into());
        } else {
            schema.remove("maximum");
        }
    }
    fn scale(self, seconds: f64) -> Result<Duration> {
        wait_timeout_from_seconds(seconds)?;
        ensure!(
            seconds >= self.minimum,
            "wait_timeout must be greater than or equal to {}",
            self.minimum
        );
        if let Some(maximum) = self.maximum {
            ensure!(
                seconds <= maximum,
                "wait_timeout must be less than or equal to {maximum}"
            );
        }
        wait_timeout_from_seconds(seconds * self.multiplier.0)
            .context("scaled wait timeout exceeds the supported duration range")
    }
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
        .map(|seconds| settings.mcp.wait_timeout.scale(seconds))
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
