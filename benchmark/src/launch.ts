import { resolve } from "node:path";
import { parseArgs } from "node:util";

import { Cause, Effect, Exit, References } from "effect";

import { isolate } from "./bootstrap/isolation";
import { createInvoker } from "./client/invoke";
import { connectPeers } from "./client/transport";
import { messages, type RunState } from "./domain";
import { dispatch } from "./orchestration/dispatch";
import { persist } from "./reporting/persistence";
import { summarize } from "./reporting/statistics";
import { loadSettings, root } from "./settings";
import { createPlan } from "./workload/plan";

const { values } = parseArgs({
  args: Bun.argv.slice(2),
  options: {
    config: { type: "string" },
    executable: { type: "string" },
    output: { type: "string" },
    "dry-run": { type: "boolean" },
    help: { type: "boolean" },
  },
});
const controller = new AbortController();
const interrupt = () => controller.abort();
process.once("SIGINT", interrupt);
process.once("SIGTERM", interrupt);
const program = Effect.gen(function* () {
  if (values.help) {
    console.log(messages.usage);
    return;
  }
  const overrides = {
    ...(values.executable ? { executable: resolve(values.executable) } : {}),
    ...(values.output ? { output: resolve(values.output) } : {}),
  };
  const settings = yield* loadSettings(
    resolve(root, values.config ?? "config/scenario.json"),
    overrides,
  );
  const plan = yield* createPlan(settings);
  if (values["dry-run"]) {
    console.log(
      JSON.stringify(
        {
          requested: settings.requestedCounts,
          effective: settings.counts,
          tabs: plan.tabs,
        },
        null,
        2,
      ),
    );
    return;
  }
  const state: RunState = {
    samples: [],
    peakInFlight: 0,
    peakBusyTabs: 0,
    cleanupErrors: [],
  };
  const run = Effect.gen(function* () {
    yield* Effect.logInfo(messages.starting, settings.counts);
    const environment = yield* isolate(settings, state);
    const peers = yield* connectPeers(
      settings,
      environment.environment,
      environment.address,
    );
    yield* Effect.logInfo(messages.running);
    yield* dispatch(
      plan,
      settings,
      environment.workspace,
      createInvoker(settings, peers, state),
      state,
    );
  }).pipe(
    Effect.scoped,
    Effect.provideService(References.MinimumLogLevel, settings.logLevel),
  );
  const outcome = yield* Effect.exit(run);
  const error = Exit.isFailure(outcome) ? Cause.pretty(outcome.cause) : null;
  const report = summarize(settings, state, error);
  yield* persist(settings.output, report);
  yield* Effect.logInfo(
    messages.saved,
    settings.output,
    report.summary.overall,
  );
  if (report.status !== "completed") {
    yield* Effect.fail(new Error(error ?? JSON.stringify(state.cleanupErrors)));
  }
});
const result = await Effect.runPromiseExit(program, {
  signal: controller.signal,
});
process.removeListener("SIGINT", interrupt);
process.removeListener("SIGTERM", interrupt);
if (Exit.isFailure(result)) {
  console.error(messages.failed, Cause.pretty(result.cause));
  process.exitCode = 1;
}
