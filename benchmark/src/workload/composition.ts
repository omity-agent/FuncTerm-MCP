import { BenchmarkFailure, type Driver } from "../domain";
import type { Settings } from "../settings";
import { waitingCommand } from "./dialects";
import type { CommandSpec } from "./plan";

function outputCommand(driver: Driver, bytes: number): string {
  switch (driver) {
    case "powershell":
      return `Write-Output ('x' * ${bytes})`;
    case "bash":
    case "zsh":
      return `printf 'x%.0s' {1..${bytes}}\nprintf '\\n'`;
    case "nu":
      return `print ('' | fill --character 'x' --width ${bytes})`;
    case "cmd": {
      const chunk = 64;
      const remainder = bytes % chunk;
      return [
        `for /L %%i in (1,1,${Math.floor(bytes / chunk)}) do @<nul set /P "=${"x".repeat(chunk)}"`,
        ...(remainder > 0 ? [`<nul set /P "=${"x".repeat(remainder)}"`] : []),
        "echo(",
        "ver >nul",
      ].join("\n");
    }
    case "bun":
      return `console.log('x'.repeat(${bytes}));`;
    case "python":
      return `print('x' * ${bytes})`;
  }
  throw new BenchmarkFailure("configuration", driver satisfies never);
}

export function composeCommand(
  driver: Driver,
  spec: CommandSpec,
  range: Settings["commandBytes"],
  readyPath?: string,
): string {
  const body = [
    outputCommand(driver, spec.outputBytes),
    ...(readyPath === undefined ? [] : [waitingCommand(driver, readyPath)]),
  ].join("\n");
  const prefix = driver === "cmd" ? "rem " : driver === "bun" ? "//" : "#";
  const required = new TextEncoder().encode(body).length + 1 + prefix.length;
  const minimum = Math.max(range.minimum, required);
  if (minimum > range.maximum) {
    throw new BenchmarkFailure(
      "commandTooShort",
      JSON.stringify({ driver, required, maximum: range.maximum }),
    );
  }
  const target =
    minimum + Math.floor(spec.lengthRatio * (range.maximum - minimum + 1));
  return `${body}\n${prefix}${"x".repeat(target - required)}`;
}
