import { afterEach, beforeEach, expect, test } from "bun:test";
import { mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { Deferred, Effect } from "effect";

import scenario from "../../config/scenario.json";
import {
  type Context,
  drivers,
  type Invoke,
  type RunState,
  type Tool,
} from "../../src/domain";
import { dispatch } from "../../src/orchestration/dispatch";
import { normalize } from "../../src/settings";
import { createPlan } from "../../src/workload/plan";
import balanced from "../fixtures/balanced.json";

const workspace = join(tmpdir(), `functerm-flow-${process.pid}`);
beforeEach(async () => {
  await rm(workspace, { recursive: true, force: true });
  await mkdir(workspace, { recursive: true });
});
afterEach(() => rm(workspace, { recursive: true, force: true }));
test("interleaves balanced tools with serial tabs and concurrent input", async () => {
  const settings = normalize({ ...scenario, ...balanced });
  const calls: { tool: Tool; context: Context }[] = [];
  const busy = new Set<string>();
  const gates = new Map<string, Deferred.Deferred<void>>();
  const state: RunState = {
    samples: [],
    peakInFlight: 0,
    peakBusyTabs: 0,
    cleanupErrors: [],
  };
  let nextTab = 0;
  const invoke: Invoke = (tool, _args, context) =>
    Effect.gen(function* () {
      calls.push({ tool, context });
      if (tool === "new_tab") {
        yield* Effect.sleep(2);
        return { tab_id: `tab-${nextTab++}` };
      }
      if (tool === "send_command") {
        const tab = context.tabId!;
        expect(busy.has(tab)).toBe(false);
        busy.add(tab);
        const work = Effect.gen(function* () {
          if (context.readyPath) {
            const gate = yield* Deferred.make<void>();
            gates.set(tab, gate);
            yield* Effect.tryPromise(() =>
              Bun.write(context.readyPath!, "ready"),
            );
            yield* Deferred.await(gate);
          } else {
            yield* Effect.sleep(3);
          }
        });
        yield* work.pipe(Effect.ensuring(Effect.sync(() => busy.delete(tab))));
      } else if (tool === "manual_write") {
        const gate = gates.get(context.tabId!);
        expect(gate).toBeDefined();
        yield* Deferred.succeed(gate!, undefined);
      }
      return {};
    });
  const plan = await Effect.runPromise(createPlan(settings));
  await Effect.runPromise(dispatch(plan, settings, workspace, invoke, state));
  for (const [tool, count] of Object.entries(settings.counts)) {
    const matching = calls.filter((call) => call.tool === tool);
    expect(matching).toHaveLength(count);
    expect(
      matching.filter((call) => call.context.transport === "stdio"),
    ).toHaveLength(count / 2);
  }
  expect(calls[0]!.tool).toBe("new_tab");
  expect(
    calls.findIndex((call, index) => index > 0 && call.tool === "new_tab"),
  ).toBeGreaterThan(1);
  expect(state.peakBusyTabs).toBe(4);
  expect(busy.size).toBe(0);
}, 10000);
test("rounds every tab combination and keeps deterministic random lengths", async () => {
  const settings = normalize({
    ...scenario,
    drivers,
    counts: { send_command: 1000, view: 100, new_tab: 50, manual_write: 10 },
  });
  expect(settings.counts.new_tab).toBe(56);
  const first = await Effect.runPromise(createPlan(settings));
  const second = await Effect.runPromise(createPlan(settings));
  expect(first.commands).toEqual(second.commands);
  const combinations = new Map<string, number>();
  for (const tab of first.tabs) {
    const key = `${tab.driver}/${tab.elevated}/${tab.transport}`;
    combinations.set(key, (combinations.get(key) ?? 0) + 1);
  }
  expect(combinations.size).toBe(28);
  expect([...combinations.values()].every((count) => count === 2)).toBe(true);
  expect(
    new Set(first.commands.map((command) => command.bytes)).size,
  ).toBeGreaterThan(100);
  for (const transportParity of [0, 1]) {
    const matching = first.commands.filter(
      (command) => command.ordinal % 2 === transportParity,
    );
    expect(matching.filter((command) => command.separate)).toHaveLength(250);
  }
});
