import { BenchmarkFailure, type Driver } from "../domain";

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
      return `(\nprintf 'ready' > ${quoted(filename)}\nIFS= read -r\n)`;
    case "nu":
      return `'ready' | save --force '${filename.replaceAll("'", "''")}'\ninput | ignore`;
    case "cmd":
      return `setlocal\ntype nul > "${filename}"\nset /P functerm_benchmark_input=\nendlocal`;
    case "bun":
      return `await (async () => {
  await Promise.resolve();
  const input = process.stdin;
  const raw = input.isRaw;
  input.pause();
  input.setRawMode(false);
  try {
    await Bun.write(${JSON.stringify(filename)}, 'ready');
    prompt('');
  } finally {
    input.setRawMode(raw);
    input.resume();
  }
})();`;
    case "python": {
      const script = `from pathlib import Path\nPath(${JSON.stringify(filename)}).write_text('ready')\ninput()`;
      return `exec(${JSON.stringify(script)}, {})`;
    }
  }
  throw new BenchmarkFailure("configuration", driver satisfies never);
}
export function keyboard(index: number): Record<string, unknown> {
  const text = `benchmark-${index}\r\n`;
  return index % 2 === 0
    ? { text }
    : { bytes: Array.from(new TextEncoder().encode(text)) };
}
