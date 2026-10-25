import { Cause, Effect, Exit, Schema } from "effect";

import {
  BenchmarkFailure,
  type Invoke,
  type RunState,
  type Sample,
} from "../domain";
import type { Settings } from "../settings";
import type { Peers } from "./transport";

const nanos = Schema.Int.check(Schema.isGreaterThanOrEqualTo(0));
const response = Schema.Struct({
  _meta: Schema.Struct({
    "functerm/processing": Schema.Struct({
      poll_ns: nanos,
      blocking_ns: nanos,
    }),
  }),
  structuredContent: Schema.Struct({
    results: Schema.Array(Schema.Struct({ result: Schema.Unknown })),
  }),
});
const command = Schema.Struct({
  command: Schema.Struct({
    finished: Schema.Boolean,
    exit_code: Schema.NullOr(Schema.Int),
  }),
});

export function createInvoker(
  settings: Settings,
  peers: Peers,
  state: RunState,
): Invoke {
  const origin = process.hrtime.bigint();
  let active = 0;
  const stamp = () => Number(process.hrtime.bigint() - origin);
  return (tool, args, context) =>
    Effect.suspend(() => {
      const sample: Sample = {
        ...context,
        sequence: state.samples.length,
        tool,
        startedNs: stamp(),
        outcome: "running",
      };
      delete sample.readyPath;
      state.samples.push(sample);
      active++;
      state.peakInFlight = Math.max(state.peakInFlight, active);
      return Effect.tryPromise({
        try: async (signal) => {
          const raw = await peers[context.transport].callTool(
            { name: tool, arguments: args },
            { timeout: settings.timeouts.requestMilliseconds, signal },
          );
          if (raw.isError) {
            throw new BenchmarkFailure("unexpected", JSON.stringify(raw));
          }
          if (!raw._meta?.["functerm/processing"]) {
            throw new BenchmarkFailure("missingTiming", tool);
          }
          const parsed = Schema.decodeUnknownSync(response)(raw);
          const measurement = parsed._meta["functerm/processing"];
          sample.pollNs = measurement.poll_ns;
          sample.blockingNs = measurement.blocking_ns;
          sample.processingNs = measurement.poll_ns + measurement.blocking_ns;
          if (
            !Number.isSafeInteger(sample.processingNs) ||
            parsed.structuredContent.results.length !== 1
          ) {
            throw new BenchmarkFailure("unexpected", JSON.stringify(raw));
          }
          const result = parsed.structuredContent.results[0]!.result;
          if (tool === "send_command") {
            const observed = Schema.decodeUnknownSync(command)(result).command;
            if (!observed.finished)
              throw new BenchmarkFailure("commandTimeout", context.tabId);
            if (observed.exit_code !== 0) {
              throw new BenchmarkFailure(
                "commandFailed",
                JSON.stringify(result),
              );
            }
          }
          return result;
        },
        catch: (cause) =>
          cause instanceof Error
            ? cause
            : new BenchmarkFailure("transportFailed", cause),
      }).pipe(
        Effect.onExit((exit) =>
          Effect.sync(() => {
            active--;
            sample.finishedNs = stamp();
            sample.outcome = Exit.isSuccess(exit)
              ? "success"
              : Cause.hasInterruptsOnly(exit.cause)
                ? "interrupted"
                : "failure";
            if (Exit.isFailure(exit)) sample.error = Cause.pretty(exit.cause);
          }),
        ),
      );
    });
}
