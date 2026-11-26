use super::{collection, invocation::Backend, operations::Operation};
use crate::runtime::{config::Settings, protocol::wait_timeout_from_seconds};
use anyhow::{Context as _, Result, ensure};
use core::str::FromStr;
use core::time::Duration;
use rmcp::{
    ErrorData,
    model::{CallToolResult, ContentBlock, JsonObject},
};
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
    #[serde(rename = "wait_timeout_default")]
    pub(crate) default_seconds: f64,
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
        wait_timeout_from_seconds(self.default_seconds)
            .context("invalid mcp.wait_timeout_default")?;
        self.scale(self.default_within_limits())
            .context("invalid effective mcp.wait_timeout_default")?;
        Ok(())
    }
    pub(super) fn apply_schema(self, schema: &mut JsonObject) {
        schema.insert("default".to_owned(), self.default_within_limits().into());
        schema.insert("minimum".to_owned(), self.minimum.into());
        if let Some(maximum) = self.maximum {
            schema.insert("maximum".to_owned(), maximum.into());
        } else {
            schema.remove("maximum");
        }
    }
    fn default_within_limits(self) -> f64 {
        let bounded = self.default_seconds.max(self.minimum);
        self.maximum.map_or(bounded, |maximum| bounded.min(maximum))
    }
    fn validate_input(self, seconds: f64) -> Result<()> {
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
        Ok(())
    }
    fn scale(self, seconds: f64) -> Result<Duration> {
        wait_timeout_from_seconds(seconds * self.multiplier.0)
            .context("scaled wait timeout exceeds the supported duration range")
    }
    fn resolve(self, timeouts: impl IntoIterator<Item = f64>) -> Result<Duration> {
        let mut largest: Option<f64> = None;
        for seconds in timeouts {
            self.validate_input(seconds)?;
            largest = Some(largest.map_or(seconds, |current| current.max(seconds)));
        }
        self.scale(largest.unwrap_or_else(|| self.default_within_limits()))
    }
}
pub(super) async fn run_timed<T>(
    settings: &Settings,
    backend: &Backend,
    entries: Vec<T>,
    timeouts: impl IntoIterator<Item = f64>,
) -> Result<CallToolResult, ErrorData>
where
    T: Operation,
{
    match settings.mcp.wait_timeout.resolve(timeouts) {
        Ok(budget) => run(settings, backend, entries, budget).await,
        Err(error) => Ok(input_error(format!("{error:#}"))),
    }
}
pub(super) async fn run<T>(
    settings: &Settings,
    backend: &Backend,
    entries: Vec<T>,
    budget: Duration,
) -> Result<CallToolResult, ErrorData>
where
    T: Operation,
{
    if let Err(message) = validate(&entries) {
        return Ok(input_error(message));
    }
    let requests = entries
        .into_iter()
        .map(|entry| entry.request(settings, budget))
        .collect::<Vec<_>>();
    if requests.iter().all(Result::is_err) {
        return collection::combine(
            requests
                .into_iter()
                .filter_map(Result::err)
                .map(Err)
                .collect(),
        );
    }
    let mut prepared = backend
        .prepare(settings)
        .await
        .map_err(super::internal_error)?;
    let started = Instant::now();
    let operations = requests.into_iter().map(|request| {
        let dispatched = request.map(|value| prepared.dispatch(settings, value, started));
        async move { T::output(dispatched?.await?) }
    });
    let results = futures_util::future::join_all(operations).await;
    collection::combine(results)
}
pub(super) fn input_error(message: String) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(message)])
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
            return Err(format!("batch contains duplicate tab_id: {tab_id}"));
        }
    }
    Ok(())
}
