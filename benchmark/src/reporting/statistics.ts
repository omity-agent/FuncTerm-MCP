import methodology from "../../config/methodology.json";
import { type Counts, type RunState, type Sample, tools } from "../domain";
import type { Settings } from "../settings";

function statistics(samples: Sample[]) {
  const values = samples
    .flatMap((sample) =>
      sample.processingNs === undefined ? [] : [sample.processingNs],
    )
    .toSorted((left, right) => left - right);
  const sum = values.reduce((total, value) => total + value, 0);
  if (!Number.isSafeInteger(sum)) throw new RangeError(String(sum));
  const percentile = (fraction: number) =>
    values.length === 0
      ? null
      : values[Math.max(0, Math.ceil(values.length * fraction) - 1)]!;
  return {
    calls: samples.length,
    successes: samples.filter((sample) => sample.outcome === "success").length,
    measured: values.length,
    sumNs: sum,
    meanNs: values.length === 0 ? null : sum / values.length,
    minimumNs: values[0] ?? null,
    p50Ns: percentile(0.5),
    p95Ns: percentile(0.95),
    p99Ns: percentile(0.99),
    maximumNs: values.at(-1) ?? null,
  };
}
function group(samples: Sample[], key: (sample: Sample) => string) {
  const grouped = new Map<string, Sample[]>();
  for (const sample of samples) {
    const name = key(sample);
    const collection = grouped.get(name) ?? [];
    collection.push(sample);
    grouped.set(name, collection);
  }
  return Object.fromEntries(
    [...grouped].map(([name, entries]) => [name, statistics(entries)]),
  );
}
export function summarize(
  settings: Settings,
  state: RunState,
  error: string | null,
) {
  const actual: Counts = {
    send_command: 0,
    view: 0,
    new_tab: 0,
    manual_write: 0,
  };
  for (const sample of state.samples) actual[sample.tool]++;
  const complete =
    error === null &&
    state.cleanupErrors.length === 0 &&
    tools.every((tool) => actual[tool] === settings.counts[tool]) &&
    state.samples.every((sample) => sample.outcome === "success");
  return {
    schemaVersion: 1,
    status: complete ? "completed" : "failed",
    generatedAt: new Date().toISOString(),
    measurement: methodology,
    configuration: settings,
    actualCounts: actual,
    peakInFlight: state.peakInFlight,
    peakBusyTabs: state.peakBusyTabs,
    error,
    cleanupErrors: state.cleanupErrors,
    summary: {
      overall: statistics(state.samples),
      byTool: group(state.samples, (sample) => sample.tool),
      byTransport: group(state.samples, (sample) => sample.transport),
      byDriver: group(state.samples, (sample) => sample.driver),
      byPrivilege: group(state.samples, (sample) =>
        sample.elevated ? "elevated" : "normal",
      ),
      byOutput: group(
        state.samples.filter((sample) => sample.tool === "send_command"),
        (sample) => (sample.separateOutput ? "separated" : "terminal"),
      ),
      byCombination: group(state.samples, (sample) =>
        [
          sample.tool,
          sample.transport,
          sample.driver,
          sample.elevated ? "elevated" : "normal",
        ].join("/"),
      ),
    },
    samples: state.samples,
  };
}
