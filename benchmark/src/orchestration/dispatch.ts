import { Effect, FiberSet, Queue, Schema } from "effect";

import {
  BenchmarkFailure,
  type Invoke,
  messages,
  type RunState,
  type Tab,
} from "../domain";
import type { Settings } from "../settings";
import type { Plan } from "../workload/plan";
import { execute } from "./interaction";

const created = Schema.Struct({ tab_id: Schema.NonEmptyString });
type Event =
  | { type: "created"; tab: Tab }
  | { type: "completed"; tab: Tab }
  | { type: "observed" };
export function dispatch(
  plan: Plan,
  settings: Settings,
  workspace: string,
  invoke: Invoke,
  state: RunState,
) {
  return Effect.gen(function* () {
    const events = yield* Queue.make<Event, Error>();
    const fibers = yield* FiberSet.make<void, Error>();
    const idle: Tab[] = [];
    const allTabs: Tab[] = [];
    let issued = 0;
    let completed = 0;
    let admitted = 0;
    let viewed = 0;
    let manualIndex = 0;
    let pending = 0;
    let busy = 0;
    const launch = (work: Effect.Effect<Event, Error>) => {
      pending++;
      return FiberSet.run(
        fibers,
        work.pipe(
          Effect.flatMap((event) => Queue.offer(events, event)),
          Effect.asVoid,
          Effect.catchCause((cause) =>
            Queue.failCause(events, cause).pipe(Effect.asVoid),
          ),
        ),
      );
    };
    while (
      completed < plan.counts.send_command ||
      pending > 0 ||
      admitted < plan.tabs.length ||
      viewed < plan.counts.view
    ) {
      while (
        admitted < plan.tabs.length &&
        (admitted === 0 || plan.tabs[admitted]!.after <= issued)
      ) {
        const admission = plan.tabs[admitted++]!;
        yield* launch(
          invoke(
            "new_tab",
            {
              exec: [
                {
                  starting_directory: workspace,
                  starting_shell: admission.driver,
                  elevated: admission.elevated,
                },
              ],
            },
            admission,
          ).pipe(
            Effect.map((raw) => ({
              type: "created" as const,
              tab: {
                id: Schema.decodeUnknownSync(created)(raw).tab_id,
                driver: admission.driver,
                elevated: admission.elevated,
              },
            })),
          ),
        );
      }
      while (idle.length > 0 && issued < plan.commands.length) {
        const reserved = plan.tabs.length - allTabs.length;
        if (plan.commands.length - issued <= reserved) break;
        const tab = idle.shift()!;
        const spec = plan.commands[issued++]!;
        const interaction = manualIndex;
        if (spec.interactive) manualIndex++;
        busy++;
        state.peakBusyTabs = Math.max(state.peakBusyTabs, busy);
        yield* launch(
          execute(tab, spec, interaction, workspace, settings, invoke).pipe(
            Effect.as({ type: "completed" as const, tab }),
          ),
        );
      }
      const dueViews = Math.floor(
        (issued * plan.counts.view) / plan.counts.send_command,
      );
      while (viewed < dueViews && allTabs.length > 0) {
        const index = viewed++;
        const tab = allTabs[index % allTabs.length]!;
        yield* launch(
          invoke(
            "view",
            {
              ids: [tab.id],
              start_token: 1,
              wait_timeout: 0,
            },
            {
              transport: index % 2 === 0 ? "stdio" : "http",
              driver: tab.driver,
              elevated: tab.elevated,
              tabId: tab.id,
            },
          ).pipe(Effect.as({ type: "observed" as const })),
        );
      }
      if (pending === 0) {
        if (completed === plan.counts.send_command) break;
        yield* Effect.fail(
          new BenchmarkFailure("stalled", { issued, admitted, completed }),
        );
      }
      const event = yield* Queue.take(events);
      pending--;
      if (event.type === "created") {
        allTabs.push(event.tab);
        idle.unshift(event.tab);
      } else if (event.type === "completed") {
        completed++;
        busy--;
        idle.push(event.tab);
        if (completed % 100 === 0) {
          yield* Effect.logInfo(messages.progress, {
            completed,
            tabs: allTabs.length,
            busy,
          });
        }
      }
    }
    yield* FiberSet.awaitEmpty(fibers);
  }).pipe(Effect.scoped);
}
