pub(super) const BOUNDARY: &str = r#"const @VAR_streamBoundary@ = (@VAR_phase@, @VAR_command@, @VAR_completion@ = {}) => {
    const @VAR_frame@ = Buffer.from(JSON.stringify({
        phase: @VAR_phase@, id: @VAR_command@.id, ...@VAR_completion@
    })).toString("base64");
    const @VAR_bytes@ = Buffer.from(`\x1b]9998;FuncTerm;${@VAR_frame@}\x07`);
    @VAR_writeSync@(1, @VAR_bytes@);
    @VAR_writeSync@(2, @VAR_bytes@);
};"#;
pub(super) const HOST: &str = r#"import { openSync, closeSync, appendFileSync } from "node:fs";
import { join } from "node:path";
import { spawnSync } from "node:child_process";
const prefix = Buffer.from("\x1b]9998;FuncTerm;");
const completions = new Map();
const helper = process.env.FUNCTERM_HELPER_EXECUTABLE;
const commandRoot = join(process.env.FUNCTERM_SESSION_ROOT, "commands");
const child = Bun.spawn({
    cmd: [process.execPath, process.argv[2]],
    stdin: "inherit", stdout: "pipe", stderr: "pipe",
});
const route = async (stream, name, terminal) => {
    let pending = Buffer.alloc(0), output, active;
    const emit = bytes => {
        if (!bytes.length) return;
        if (output !== undefined) appendFileSync(output, bytes);
        terminal.write(bytes);
    };
    try {
        for await (const chunk of stream) {
            pending = Buffer.concat([pending, Buffer.from(chunk)]);
            while (pending.length) {
                const start = pending.indexOf(prefix);
                if (start < 0) {
                    let retained = Math.min(prefix.length - 1, pending.length);
                    while (retained && !pending.subarray(-retained).equals(prefix.subarray(0, retained))) retained--;
                    emit(pending.subarray(0, pending.length - retained));
                    pending = pending.subarray(pending.length - retained);
                    break;
                }
                emit(pending.subarray(0, start));
                pending = pending.subarray(start);
                const end = pending.indexOf(7, prefix.length);
                if (end < 0) {
                    if (pending.length > 65536) throw new Error("oversized stream boundary");
                    break;
                }
                const frame = JSON.parse(Buffer.from(pending.subarray(prefix.length, end).toString(), "base64").toString());
                pending = pending.subarray(end + 1);
                if (!/^command-[a-z0-9]+$/.test(frame.id)) throw new Error("invalid stream command id");
                if (frame.phase === "start") {
                    if (active) throw new Error("stream command already active");
                    active = frame.id;
                    output = openSync(join(commandRoot, active, name), "w");
                } else if (frame.phase === "end") {
                    if (active !== frame.id) throw new Error("stream completion does not match active command");
                    closeSync(output);
                    output = undefined;
                    active = undefined;
                    const previous = completions.get(frame.id);
                    if (!previous) {
                        completions.set(frame.id, name);
                    } else {
                        if (previous === name) throw new Error("duplicate stream completion");
                        completions.delete(frame.id);
                        const result = spawnSync(helper, [
                            "internal-write-done", "--command-id", frame.id,
                            "--directory", join(commandRoot, frame.id),
                            "--exit-code", String(frame.exitCode),
                            "--time-consumption", frame.elapsed, "--cwd", frame.cwd,
                        ], { stdio: "inherit" });
                        if (result.error) throw result.error;
                        if (result.status !== 0) throw new Error(`stream completion helper exited with ${result.status}`);
                    }
                } else {
                    throw new Error("unknown stream boundary phase");
                }
            }
        }
        emit(pending);
    } finally {
        if (output !== undefined) closeSync(output);
    }
};
try {
    await Promise.all([
        route(child.stdout, "stdout.txt", process.stdout),
        route(child.stderr, "stderr.txt", process.stderr),
    ]);
    process.exit(await child.exited);
} catch (error) {
    console.error(error);
    child.kill();
    process.exit(1);
}
"#;
