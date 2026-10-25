import type { Effect } from "effect";

import messages from "../config/locales/zh-CN.json";

export const drivers = [
  "powershell",
  "bash",
  "nu",
  "zsh",
  "cmd",
  "bun",
  "python",
] as const;
export type Driver = (typeof drivers)[number];
export type Transport = "stdio" | "http";
export type Tool = "send_command" | "view" | "new_tab" | "manual_write";
export const tools = [
  "send_command",
  "view",
  "new_tab",
  "manual_write",
] as const;
export type Counts = Record<Tool, number>;
export type Tab = { id: string; driver: Driver; elevated: boolean };
export type Context = {
  transport: Transport;
  driver: Driver;
  elevated: boolean;
  tabId?: string;
  commandBytes?: number;
  payloadBytes?: number;
  separateOutput?: boolean;
  readyPath?: string;
};
export type Sample = Context & {
  sequence: number;
  tool: Tool;
  startedNs: number;
  finishedNs?: number;
  pollNs?: number;
  blockingNs?: number;
  processingNs?: number;
  outputBytes?: number;
  outcome: "running" | "success" | "failure" | "interrupted";
  error?: string;
};
export type Invoke = (
  tool: Tool,
  args: Record<string, unknown>,
  context: Context,
) => Effect.Effect<unknown, Error>;
export type RunState = {
  samples: Sample[];
  peakInFlight: number;
  peakBusyTabs: number;
  cleanupErrors: string[];
};

export class BenchmarkFailure extends Error {
  constructor(
    readonly code: keyof typeof messages,
    readonly detail: unknown,
  ) {
    super(`${messages[code]}: ${String(detail)}`);
    this.name = "BenchmarkFailure";
  }
}

export { messages };
