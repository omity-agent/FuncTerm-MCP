use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, COMMAND_INPUT_DIRECTORY,
    COMMAND_OUTPUT_DIRECTORY, COMMAND_WORKING_DIRECTORY_FILE, DISPATCH_FILE, HELPER_EXECUTABLE_ENV,
    SESSION_COMMANDS_DIRECTORY, SESSION_STATE_DIRECTORY, STDERR_FILE, STDOUT_FILE,
};
#[expect(
    clippy::uninlined_format_args,
    reason = "explicit aliases keep Rust format fields distinct from embedded JavaScript templates"
)]
pub(super) fn script(cwd: &str, ready: &str) -> String {
    format!(
        r#"import {{ spawnSync }} from "node:child_process";
import {{ existsSync, readFileSync, rmSync, watch, writeFileSync }} from "node:fs";
import {{ join, delimiter }} from "node:path";
import {{ start }} from "node:repl";
import {{ format }} from "node:util";
process.chdir({cwd});
process.env.FUNCTERM_CURRENT_SHELL = "bun";
const server = start({{ prompt: "> " }});
const evaluate = server.eval;
let activeCommand;
let dispatching = false;
let inputDepth = 0;
let pendingEvaluations = 0;
let deferredPrompt;
let evaluationReturned = false;
const runHelper = args => {{
    const helper = process.env.{helper_env};
    if (!helper) throw new Error("{helper_env} is not set");
    const result = spawnSync(helper, args, {{ env: process.env, stdio: "inherit" }});
    if (result.error) throw result.error;
    if (result.status !== 0) throw new Error(`FuncTerm helper exited with ${{result.status}}`);
}};
const prependShimPath = () => {{
    const shim = process.env.FUNCTERM_SHIM_DIR;
    if (!shim) return;
    process.env.PATH = [shim, ...(process.env.PATH ?? "").split(delimiter).filter(value => value && value !== shim)].join(delimiter);
}};
const captureOutput = () => {{
    const stdout = [], stderr = [];
    const stdoutWrite = process.stdout.write.bind(process.stdout);
    const stderrWrite = process.stderr.write.bind(process.stderr);
    const write = (original, chunks) => (chunk, ...args) => {{
        chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk, typeof args[0] === "string" ? args[0] : undefined));
        return original(chunk, ...args);
    }};
    process.stdout.write = write(stdoutWrite, stdout);
    process.stderr.write = write(stderrWrite, stderr);
    const consoleMethods = [];
    for (const consoleObject of new Set([console, server.context.console])) {{
        for (const [name, target] of [["log", process.stdout], ["info", process.stdout], ["warn", process.stderr], ["error", process.stderr]]) {{
            consoleMethods.push([consoleObject, name, consoleObject[name]]);
            consoleObject[name] = (...values) => target.write(`${{format(...values)}}\n`);
        }}
    }}
    return {{
        stdout, stderr,
        restore() {{
            process.stdout.write = stdoutWrite;
            process.stderr.write = stderrWrite;
            for (const [consoleObject, name, method] of consoleMethods) consoleObject[name] = method;
        }}
    }};
}};
const restoreEnvironment = (name, value) => value === undefined ? delete process.env[name] : process.env[name] = value;
const finishCommand = () => {{
    const command = activeCommand;
    if (!command) return;
    activeCommand = undefined;
    command.capture.restore();
    writeFileSync(join(command.output, "{stdout_file}"), Buffer.concat(command.capture.stdout));
    writeFileSync(join(command.output, "{stderr_file}"), Buffer.concat(command.capture.stderr));
    const elapsed = `${{Math.max(1, Math.ceil(performance.now() - command.started))}}ms`;
    runHelper(["internal-write-done", "--command-id", command.id, "--exit-code", command.failed ? "1" : "0", "--time-consumption", elapsed, "--cwd", process.cwd(), "--directory", command.directory]);
    restoreEnvironment("{command_id_env}", command.previousId);
    restoreEnvironment("{command_directory_env}", command.previousDirectory);
}};
const displayPrompt = server.displayPrompt.bind(server);
server.displayPrompt = (...args) => {{
    const nativeError = activeCommand && server.context._error !== activeCommand.previousError;
    if (nativeError) {{
        activeCommand.failed = true;
        pendingEvaluations = 0;
        evaluationReturned = true;
    }}
    if (activeCommand && (inputDepth > 0 || pendingEvaluations > 0 || !evaluationReturned)) {{
        deferredPrompt = args;
        return;
    }}
    finishCommand();
    return displayPrompt(...args);
}};
const finishDeferredPrompt = () => {{
    if (!activeCommand || inputDepth > 0 || pendingEvaluations > 0 || !evaluationReturned || !deferredPrompt) return;
    const args = deferredPrompt;
    deferredPrompt = undefined;
    finishCommand();
    displayPrompt(...args);
}};
server.eval = (source, context, file, callback) => {{
    pendingEvaluations += 1;
    let callbackCalled = false;
    const finishEvaluation = (error, value) => {{
        if (callbackCalled) return;
        callbackCalled = true;
        if (activeCommand && error) activeCommand.failed = true;
        pendingEvaluations -= 1;
        callback(error, value);
        finishDeferredPrompt();
    }};
    try {{
        const returned = evaluate.call(server, source, context, file, finishEvaluation);
        if (returned && typeof returned.then === "function") {{
            returned.then(value => finishEvaluation(null, value), error => finishEvaluation(error));
        }}
    }} finally {{
        evaluationReturned = true;
        finishDeferredPrompt();
    }}
}};
server.on("exit", () => {{
    finishCommand();
    watcher.close();
}});
const stateDirectory = join(process.env.FUNCTERM_SESSION_ROOT, "{session_state}");
const dispatchFile = join(stateDirectory, "{dispatch_file}");
const dispatch = () => {{
    if (dispatching || activeCommand) return;
    dispatching = true;
    try {{
        const commandId = readFileSync(dispatchFile, "utf8");
        rmSync(dispatchFile);
        const directory = join(process.env.FUNCTERM_SESSION_ROOT, "{commands_directory}", commandId);
        const input = join(directory, "{input_directory}");
        const output = join(directory, "{output_directory}");
        const source = readFileSync(join(input, "{command_file}"), "utf8");
        const workingDirectory = readFileSync(join(input, "{working_directory_file}"), "utf8");
        const previousId = process.env.{command_id_env};
        const previousDirectory = process.env.{command_directory_env};
        const previousError = server.context._error;
        process.env.{command_id_env} = commandId;
        process.env.{command_directory_env} = directory;
        runHelper(["internal-ensure-shims", "--directory", process.env.FUNCTERM_SHIM_DIR]);
        prependShimPath();
        process.chdir(workingDirectory);
        runHelper(["internal-write-start", "--command-id", commandId, "--directory", directory]);
        activeCommand = {{
            id: commandId, directory, output, previousId, previousDirectory, previousError,
            started: performance.now(), failed: false, capture: captureOutput()
        }};
        evaluationReturned = true;
        inputDepth += 1;
        try {{
            server.write(source.endsWith("\n") ? source : `${{source}}\n`);
        }} finally {{
            inputDepth -= 1;
        }}
        finishDeferredPrompt();
        server.resume();
    }} catch (error) {{
        console.error(error);
        server.close();
    }} finally {{
        dispatching = false;
    }}
}};
const watcher = watch(stateDirectory, () => {{
    if (!existsSync(dispatchFile)) return;
    try {{
        dispatch();
    }} catch (error) {{
        console.error(error);
        server.close();
    }}
}});
server.resume();
writeFileSync({ready}, "");
"#,
        helper_env = HELPER_EXECUTABLE_ENV,
        session_state = SESSION_STATE_DIRECTORY,
        dispatch_file = DISPATCH_FILE,
        commands_directory = SESSION_COMMANDS_DIRECTORY,
        input_directory = COMMAND_INPUT_DIRECTORY,
        output_directory = COMMAND_OUTPUT_DIRECTORY,
        command_file = COMMAND_FILE,
        working_directory_file = COMMAND_WORKING_DIRECTORY_FILE,
        command_id_env = COMMAND_ID_ENV,
        command_directory_env = COMMAND_DIRECTORY_ENV,
        stdout_file = STDOUT_FILE,
        stderr_file = STDERR_FILE,
    )
}
