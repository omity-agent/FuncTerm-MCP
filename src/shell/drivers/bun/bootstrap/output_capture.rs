pub(super) const SCRIPT: &str = r#"const captureOutput = () => {
    const stdout = [], stderr = [];
    const stdoutWrite = process.stdout.write.bind(process.stdout);
    const stderrWrite = process.stderr.write.bind(process.stderr);
    const write = (original, chunks, stdout) => (chunk, ...args) => {
        const bytes = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, typeof args[0] === "string" ? args[0] : undefined);
        let captured = bytes;
        if (stdout && activeCommand?.readingInput) {
            captured = undefined;
            if (activeCommand.nativeInput) {
                const lineEnd = bytes.indexOf(10);
                if (lineEnd >= 0) {
                    activeCommand.readingInput = false;
                    captured = bytes.subarray(lineEnd + 1);
                }
            }
        }
        if (captured?.length) chunks.push(captured);
        return original(chunk, ...args);
    };
    process.stdout.write = write(stdoutWrite, stdout, true);
    process.stderr.write = write(stderrWrite, stderr, false);
    const consoleMethods = [];
    for (const consoleObject of new Set([console, server.context.console])) {
        for (const [name, target] of [["log", process.stdout], ["info", process.stdout], ["warn", process.stderr], ["error", process.stderr]]) {
            consoleMethods.push([consoleObject, name, consoleObject[name]]);
            consoleObject[name] = (...values) => target.write(`${format(...values)}\n`);
        }
    }
    return {
        stdout, stderr,
        restore() {
            process.stdout.write = stdoutWrite;
            process.stderr.write = stderrWrite;
            for (const [consoleObject, name, method] of consoleMethods) consoleObject[name] = method;
        }
    };
};"#;
