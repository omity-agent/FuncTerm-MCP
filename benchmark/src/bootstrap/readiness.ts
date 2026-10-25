import { createInterface } from "node:readline";
import { Readable } from "node:stream";

import { Effect, Schema } from "effect";

import { BenchmarkFailure } from "../domain";
import type { Settings } from "../settings";

const ready = Schema.Struct({
  Ready: Schema.Struct({ http_address: Schema.NonEmptyString }),
});
export function readAddress(stdout: ReadableStream<Uint8Array>) {
  return Effect.tryPromise({
    try: async (signal) => {
      const lines = createInterface({ input: Readable.from(stdout) });
      const abort = () => lines.close();
      signal.addEventListener("abort", abort, { once: true });
      try {
        const first = await lines[Symbol.asyncIterator]().next();
        if (first.done) throw new BenchmarkFailure("startupFailed", "EOF");
        const value: unknown = JSON.parse(first.value);
        return Schema.decodeUnknownSync(ready)(value).Ready.http_address;
      } finally {
        signal.removeEventListener("abort", abort);
        lines.close();
      }
    },
    catch: (cause) => new BenchmarkFailure("startupFailed", cause),
  });
}
export function waitForInput(path: string, settings: Settings) {
  return Effect.gen(function* () {
    while (
      !(yield* Effect.tryPromise({
        try: () => Bun.file(path).exists(),
        catch: (cause) => new BenchmarkFailure("readyTimeout", cause),
      }))
    ) {
      yield* Effect.sleep(settings.timeouts.readyPollMilliseconds);
    }
  }).pipe(
    Effect.timeoutOrElse({
      duration: settings.timeouts.commandSeconds * 1000,
      orElse: () => Effect.fail(new BenchmarkFailure("readyTimeout", path)),
    }),
  );
}
