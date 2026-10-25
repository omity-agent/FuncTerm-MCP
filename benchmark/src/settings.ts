import { resolve } from "node:path";

import { Effect, Schema } from "effect";

import { BenchmarkFailure, type Counts, drivers } from "./domain";

const positive = Schema.Int.check(
  Schema.isGreaterThan(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const count = Schema.Int.check(
  Schema.isGreaterThanOrEqualTo(0),
  Schema.isLessThanOrEqualTo(Number.MAX_SAFE_INTEGER),
);
const definition = Schema.Struct({
  executable: Schema.NonEmptyString,
  drivers: Schema.Array(Schema.Literals(drivers)),
  counts: Schema.Struct({
    send_command: positive,
    view: count,
    new_tab: positive,
    manual_write: count,
  }),
  commandBytes: Schema.Struct({ minimum: positive, maximum: positive }),
  timeouts: Schema.Struct({
    commandSeconds: positive,
    requestMilliseconds: positive,
    startupMilliseconds: positive,
    cleanupMilliseconds: positive,
    readyPollMilliseconds: positive,
  }),
  seed: Schema.Int,
  output: Schema.NonEmptyString,
  logLevel: Schema.Literals(["Debug", "Info", "Warning", "Error"]),
});
type Scenario = typeof definition.Type;
export type Settings = Scenario & {
  requestedCounts: Counts;
  counts: Counts;
};
export const root = resolve(import.meta.dir, "..");
function rounded(value: number, quantum: number): number {
  if (value === 0) return 0;
  const result = Math.max(1, Math.round(value / quantum)) * quantum;
  if (!Number.isSafeInteger(result)) {
    throw new BenchmarkFailure("configuration", { value, quantum });
  }
  return result;
}
export function normalize(input: unknown): Settings {
  const scenario = Schema.decodeUnknownSync(definition)(input);
  if (
    scenario.drivers.length === 0 ||
    new Set(scenario.drivers).size !== scenario.drivers.length ||
    scenario.commandBytes.maximum < scenario.commandBytes.minimum ||
    scenario.timeouts.requestMilliseconds <=
      scenario.timeouts.commandSeconds * 1000
  ) {
    throw new BenchmarkFailure("configuration", JSON.stringify(input));
  }
  const counts: Counts = {
    send_command: rounded(scenario.counts.send_command, 4),
    view: rounded(scenario.counts.view, 2),
    new_tab: rounded(scenario.counts.new_tab, scenario.drivers.length * 4),
    manual_write: rounded(scenario.counts.manual_write, 2),
  };
  if (
    counts.manual_write > counts.send_command ||
    counts.new_tab > counts.send_command
  ) {
    throw new BenchmarkFailure("configuration", JSON.stringify(counts));
  }
  return {
    ...scenario,
    executable: resolve(root, scenario.executable),
    output: resolve(root, scenario.output),
    requestedCounts: { ...scenario.counts },
    counts,
  };
}
export function loadSettings(path: string, overrides: Record<string, unknown>) {
  return Effect.tryPromise({
    try: async () => {
      const input: unknown = await Bun.file(path).json();
      if (typeof input !== "object" || input === null) {
        throw new BenchmarkFailure("configuration", path);
      }
      return normalize({ ...input, ...overrides });
    },
    catch: (cause) => new BenchmarkFailure("configuration", cause),
  });
}
