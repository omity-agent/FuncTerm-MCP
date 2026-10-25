import { BenchmarkFailure, type Driver } from "../domain";

export function emptyCommand(driver: Driver, bytes: number): string {
  const prefix = driver === "cmd" ? "rem " : driver === "bun" ? "//" : "#";
  return prefix + "x".repeat(Math.max(0, bytes - prefix.length));
}
function quoted(text: string): string {
  return `'${text.replaceAll("'", "'\\''")}'`;
}
export function waitingCommand(driver: Driver, ready: string): string {
  const filename = ready.replaceAll("\\", "/");
  switch (driver) {
    case "powershell":
      return `[System.IO.File]::WriteAllText('${filename.replaceAll("'", "''")}', 'ready')\n$null = [Console]::ReadLine()`;
    case "bash":
    case "zsh":
      return `printf 'ready' > ${quoted(filename)}\nIFS= read -r functerm_benchmark_input`;
    case "nu":
      return `'ready' | save --force '${filename.replaceAll("'", "''")}'\ninput | ignore`;
    case "cmd":
      return `type nul > "${filename}"\nset /p functerm_benchmark_input=`;
    case "bun":
      return `await Bun.write(${JSON.stringify(filename)}, 'ready');\nawait new Promise(resolve => process.stdin.once('data', resolve));`;
    case "python":
      return `from pathlib import Path\n_ = Path(${JSON.stringify(filename)}).write_text('ready')\n_ = input()`;
  }
  throw new BenchmarkFailure("configuration", driver satisfies never);
}
export function keyboard(index: number): Record<string, unknown> {
  const text = `benchmark-${index}\r\n`;
  return index % 2 === 0
    ? { text }
    : { bytes: Array.from(new TextEncoder().encode(text)) };
}
