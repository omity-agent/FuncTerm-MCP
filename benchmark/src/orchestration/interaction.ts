import { join } from "node:path";

import { Effect } from "effect";

import { waitForInput } from "../bootstrap/readiness";
import type { Invoke, Tab, Transport } from "../domain";
import type { Settings } from "../settings";
import { emptyCommand, keyboard, waitingCommand } from "../workload/dialects";
import type { CommandSpec } from "../workload/plan";

export function execute(
  tab: Tab,
  spec: CommandSpec,
  manualIndex: number,
  workspace: string,
  settings: Settings,
  invoke: Invoke,
) {
  return Effect.gen(function* () {
    const readyPath = join(workspace, `ready-${spec.ordinal}`);
    const source = spec.interactive
      ? waitingCommand(tab.driver, readyPath)
      : emptyCommand(tab.driver, spec.bytes);
    const transport: Transport = spec.ordinal % 2 === 0 ? "stdio" : "http";
    const context = {
      transport,
      driver: tab.driver,
      elevated: tab.elevated,
      tabId: tab.id,
      commandBytes: new TextEncoder().encode(source).length,
      separateOutput: spec.separate,
      ...(spec.interactive ? { readyPath } : {}),
    };
    const send = invoke(
      "send_command",
      {
        exec: [
          { tab_id: tab.id, command: source, separate_output: spec.separate },
        ],
        wait_timeout: settings.timeouts.commandSeconds,
      },
      context,
    );
    if (!spec.interactive) {
      yield* send;
      return;
    }
    const manual = Effect.gen(function* () {
      yield* waitForInput(readyPath, settings);
      yield* invoke(
        "manual_write",
        {
          exec: [{ tab_id: tab.id, ...keyboard(manualIndex) }],
          wait_timeout: 0,
        },
        {
          ...context,
          transport: manualIndex % 2 === 0 ? "stdio" : "http",
        },
      );
    });
    yield* Effect.all([send, manual], { concurrency: "unbounded" });
  });
}
