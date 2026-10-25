import { afterEach, beforeEach, expect, test } from "bun:test";
import { mkdir, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";

import { Effect, Exit } from "effect";

import scenario from "../../config/scenario.json";
import { BenchmarkFailure, type Invoke, type RunState } from "../../src/domain";
import { dispatch } from "../../src/orchestration/dispatch";
import { persist } from "../../src/reporting/persistence";
import { summarize } from "../../src/reporting/statistics";
import { normalize } from "../../src/settings";
import { createPlan } from "../../src/workload/plan";
import rejected from "../fixtures/rejected.json";

const workspace = join(tmpdir(), `functerm-abort-${process.pid}`);
beforeEach(async () => {
  await rm(workspace, { recursive: true, force: true });
  await mkdir(workspace, { recursive: true });
});
afterEach(() => rm(workspace, { recursive: true, force: true }));
test("stops dispatch on a send timeout and persists the partial report", async () => {
  const settings = normalize({ ...scenario, ...rejected });
  const state: RunState = {
    samples: [],
    peakInFlight: 0,
    peakBusyTabs: 0,
    cleanupErrors: [],
  };
  let sends = 0;
  const invoke: Invoke = (tool) =>
    Effect.suspend(() => {
      if (tool === "send_command") {
        sends++;
        return Effect.fail(new BenchmarkFailure("commandTimeout", "injected"));
      }
      return Effect.succeed({ tab_id: "tab-only" });
    });
  const plan = await Effect.runPromise(createPlan(settings));
  const result = await Effect.runPromiseExit(
    dispatch(plan, settings, workspace, invoke, state),
  );
  expect(Exit.isFailure(result)).toBe(true);
  expect(sends).toBe(1);
  const path = join(workspace, "partial.json");
  await Effect.runPromise(
    persist(path, summarize(settings, state, "injected timeout")),
  );
  const report: unknown = await Bun.file(path).json();
  expect(report).toMatchObject({ status: "failed", error: "injected timeout" });
});
