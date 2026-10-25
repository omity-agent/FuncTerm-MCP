import { randomUUID } from "node:crypto";
import { mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { Effect } from "effect";

import { BenchmarkFailure, messages, type RunState } from "../domain";
import type { Settings } from "../settings";
import { readAddress } from "./readiness";

export function isolate(settings: Settings, state: RunState) {
  return Effect.gen(function* () {
    const workspace = yield* Effect.acquireRelease(
      Effect.tryPromise({
        try: () => mkdtemp(join(tmpdir(), "functerm-benchmark-")),
        catch: (cause) => new BenchmarkFailure("startupFailed", cause),
      }),
      (path) =>
        Effect.tryPromise({
          try: () => rm(path, { recursive: true, force: true }),
          catch: (cause) => new BenchmarkFailure("shutdown", cause),
        }).pipe(Effect.orDie),
    );
    const environment = Object.fromEntries(
      Object.entries(process.env).filter(
        (entry): entry is [string, string] =>
          entry[1] !== undefined &&
          !entry[0].startsWith("FUNCTERM_DAEMON_READY_"),
      ),
    );
    environment.FUNCTERM_DAEMON_SERVICE_NAME = `functerm/benchmark/${randomUUID()}`;
    environment.FUNCTERM_MCP_HTTP_BIND = "127.0.0.1:0";
    environment.FUNCTERM_SHELL_LOAD_PROFILE = "false";
    environment.FUNCTERM_BENCHMARK_PROCESSING_TIMING = "true";
    const daemon = yield* Effect.acquireRelease(
      Effect.try({
        try: () =>
          Bun.spawn([settings.executable, "internal-run-daemon"], {
            env: { ...environment, FUNCTERM_DAEMON_READY_STDOUT: "1" },
            stdout: "pipe",
            stderr: "inherit",
            stdin: "ignore",
          }),
        catch: (cause) => new BenchmarkFailure("startupFailed", cause),
      }),
      (child) =>
        Effect.gen(function* () {
          yield* Effect.logInfo(messages.cleanup);
          const cleanup = Effect.tryPromise({
            try: async (signal) => {
              if (
                !environment.FUNCTERM_DAEMON_SERVICE_NAME?.startsWith(
                  "functerm/benchmark/",
                )
              ) {
                throw new BenchmarkFailure("unsafeService", environment);
              }
              if (child.exitCode !== null) return;
              const stop = Bun.spawn([settings.executable, "daemon", "stop"], {
                env: environment,
                stdout: "pipe",
                stderr: "pipe",
              });
              const abort = () => {
                stop.kill();
                child.kill();
              };
              signal.addEventListener("abort", abort, { once: true });
              try {
                const code = await stop.exited;
                const stderr = await new Response(stop.stderr).text();
                if (code !== 0) throw new BenchmarkFailure("shutdown", stderr);
                await child.exited;
              } finally {
                signal.removeEventListener("abort", abort);
                if (child.exitCode === null) child.kill();
              }
            },
            catch: (cause) => new BenchmarkFailure("shutdown", cause),
          }).pipe(Effect.timeout(settings.timeouts.cleanupMilliseconds));
          yield* cleanup.pipe(
            Effect.catchCause((cause) =>
              Effect.gen(function* () {
                const error = String(cause);
                state.cleanupErrors.push(error);
                if (child.exitCode === null) child.kill();
                yield* Effect.logError(messages.shutdown, error);
              }),
            ),
          );
        }),
    );
    const address = yield* readAddress(daemon.stdout).pipe(
      Effect.timeout(settings.timeouts.startupMilliseconds),
    );
    return { workspace, environment, address };
  });
}
