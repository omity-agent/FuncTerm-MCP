import { randomUUID } from "node:crypto";
import { mkdir, rename, rm } from "node:fs/promises";
import { dirname } from "node:path";

import { Effect } from "effect";

import { BenchmarkFailure } from "../domain";

export function persist(path: string, data: unknown) {
  return Effect.tryPromise({
    try: async () => {
      await mkdir(dirname(path), { recursive: true });
      const temporary = `${path}.${randomUUID()}.tmp`;
      try {
        await Bun.write(temporary, JSON.stringify(data, null, 2) + "\n");
        await rename(temporary, path);
      } finally {
        await rm(temporary, { force: true });
      }
    },
    catch: (cause) => new BenchmarkFailure("failed", cause),
  });
}
