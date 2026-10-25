import { afterEach, beforeEach, expect, test } from "bun:test";
import { mkdir, readdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { Effect, Schema } from "effect";

import scenario from "../../config/scenario.json";
import { isolate } from "../../src/bootstrap/isolation";
import { createInvoker } from "../../src/client/invoke";
import { connectPeers } from "../../src/client/transport";
import type { RunState } from "../../src/domain";
import { execute } from "../../src/orchestration/interaction";
import { normalize } from "../../src/settings";

const workspace = join(tmpdir(), `functerm-roundtrip-${process.pid}`);
beforeEach(async () => {
  await rm(workspace, { recursive: true, force: true });
  await mkdir(workspace, { recursive: true });
});
afterEach(() => rm(workspace, { recursive: true, force: true }));
test(
  "shell payloads round-trip input and output with only readiness files",
  async () => {
    const settings = normalize({
      ...scenario,
      executable:
        process.env.FUNCTERM_BENCHMARK_EXECUTABLE ?? scenario.executable,
    });
    const state: RunState = {
      samples: [],
      peakInFlight: 0,
      peakBusyTabs: 0,
      cleanupErrors: [],
    };
    await Effect.runPromise(
      Effect.gen(function* () {
        const environment = yield* isolate(settings, state);
        const peers = yield* connectPeers(
          settings,
          environment.environment,
          environment.address,
        );
        const invoke = createInvoker(settings, peers, state);
        for (const [driverIndex, driver] of settings.drivers.entries()) {
          const context = {
            transport: "stdio" as const,
            driver,
            elevated: false,
          };
          const raw = yield* invoke(
            "new_tab",
            {
              exec: [
                {
                  starting_directory: workspace,
                  starting_shell: driver,
                  elevated: false,
                },
              ],
            },
            context,
          );
          const tab = {
            id: Schema.decodeUnknownSync(
              Schema.Struct({ tab_id: Schema.NonEmptyString }),
            )(raw).tab_id,
            driver,
            elevated: false,
          };
          for (let index = 0; index < 4; index++) {
            for (const interactive of [true, false]) {
              yield* execute(
                tab,
                {
                  ordinal: driverIndex * 4 + index,
                  lengthRatio: index / 4,
                  outputBytes:
                    index % 2 === 0
                      ? settings.commandBytes.minimum
                      : settings.commandBytes.maximum,
                  separate: index < 2,
                  interactive,
                },
                index,
                workspace,
                settings,
                invoke,
              );
            }
          }
        }
      }).pipe(Effect.scoped),
    );
    expect(state.cleanupErrors).toEqual([]);
    expect(state.samples).toHaveLength(settings.drivers.length * 13);
    expect(state.samples.every((sample) => sample.outcome === "success")).toBe(
      true,
    );
    const sends = state.samples.filter(
      (sample) => sample.tool === "send_command",
    );
    for (const sample of sends) {
      expect(sample.commandBytes).toBeGreaterThanOrEqual(
        settings.commandBytes.minimum,
      );
      expect(sample.commandBytes).toBeLessThanOrEqual(
        settings.commandBytes.maximum,
      );
      expect(sample.outputBytes).toBeGreaterThanOrEqual(sample.payloadBytes!);
    }
    expect((await readdir(workspace)).toSorted()).toEqual(
      Array.from(
        { length: settings.drivers.length * 4 },
        (_, index) => `ready-${index}`,
      ).toSorted(),
    );
  },
  scenario.timeouts.startupMilliseconds,
);
