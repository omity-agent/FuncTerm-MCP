import { Effect, Random } from "effect";

import type { Context, Counts, Driver } from "../domain";
import type { Settings } from "../settings";

export type Admission = Context & { ordinal: number; after: number };
export type CommandSpec = {
  ordinal: number;
  bytes: number;
  separate: boolean;
  interactive: boolean;
};
export type Plan = {
  tabs: Admission[];
  commands: CommandSpec[];
  counts: Counts;
};
function tabPlan(
  shells: readonly Driver[],
  total: number,
  sends: number,
): Admission[] {
  return Array.from({ length: total }, (_, ordinal) => {
    const cycle = Math.floor(ordinal / shells.length);
    return {
      ordinal,
      after: Math.floor((ordinal * (sends - total)) / total),
      driver: shells[ordinal % shells.length]!,
      elevated: cycle % 2 === 1,
      transport:
        (Math.floor(cycle / 2) + (ordinal % shells.length)) % 2 === 0
          ? "stdio"
          : "http",
    };
  });
}
export function createPlan(settings: Settings) {
  return Effect.gen(function* () {
    const { counts } = settings;
    const interactions = new Set(
      Array.from({ length: counts.manual_write }, (_, index) =>
        Math.floor(((index + 0.5) * counts.send_command) / counts.manual_write),
      ),
    );
    const commands: CommandSpec[] = [];
    for (let ordinal = 0; ordinal < counts.send_command; ordinal++) {
      commands.push({
        ordinal,
        bytes: yield* Random.nextIntBetween(
          settings.commandBytes.minimum,
          settings.commandBytes.maximum,
        ),
        separate: Math.floor(ordinal / 2) % 2 === 0,
        interactive: interactions.has(ordinal),
      });
    }
    return {
      counts,
      commands,
      tabs: tabPlan(settings.drivers, counts.new_tab, counts.send_command),
    };
  }).pipe(Random.withSeed(settings.seed));
}
