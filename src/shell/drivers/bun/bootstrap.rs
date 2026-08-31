mod output_capture;
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, COMMAND_INPUT_DIRECTORY,
    COMMAND_OUTPUT_DIRECTORY, COMMAND_WORKING_DIRECTORY_FILE, DISPATCH_FILE, HELPER_EXECUTABLE_ENV,
    SESSION_COMMANDS_DIRECTORY, SESSION_STATE_DIRECTORY, STDERR_FILE, STDOUT_FILE,
};
pub(super) fn script(cwd: &str, ready: &str) -> String {
    let bootstrap = format!(
        r#"import {{ spawnSync as @VAR_spawnSync@ }} from "node:child_process";
	import {{ existsSync as @VAR_existsSync@, readFileSync as @VAR_readFileSync@, rmSync as @VAR_rmSync@, watch as @VAR_watch@, writeFileSync as @VAR_writeFileSync@ }} from "node:fs";
	import {{ join as @VAR_join@, delimiter as @VAR_delimiter@ }} from "node:path";
	import {{ Recoverable as @VAR_Recoverable@, start as @VAR_start@ }} from "node:repl";
	import {{ format as @VAR_format@, inspect as @VAR_inspect@ }} from "node:util";
	process.chdir({cwd});
	process.env.FUNCTERM_CURRENT_SHELL = "bun";
	const @VAR_server@ = @VAR_start@({{ prompt: "> " }});
	const @VAR_evaluate@ = @VAR_server@.eval;
	let @VAR_activeCommand@;
	let @VAR_dispatching@ = false;
	let @VAR_inputDepth@ = 0;
	let @VAR_pendingEvaluations@ = 0;
	let @VAR_deferredPrompt@;
	let @VAR_evaluationReturned@ = false;
	const @VAR_runHelper@ = @VAR_args@ => {{
	    const @VAR_helper@ = process.env.{helper_env};
	    if (!@VAR_helper@) throw new Error("{helper_env} is not set");
	    const @VAR_result@ = @VAR_spawnSync@(@VAR_helper@, @VAR_args@, {{ env: process.env, stdio: "inherit" }});
	    if (@VAR_result@.error) throw @VAR_result@.error;
	    if (@VAR_result@.status !== 0) throw new Error(`FuncTerm helper exited with ${{@VAR_result@.status}}`);
	}};
	const @VAR_prependShimPath@ = () => {{
	    const @VAR_shim@ = process.env.FUNCTERM_SHIM_DIR;
	    if (!@VAR_shim@) return;
	    process.env.PATH = [@VAR_shim@, ...(process.env.PATH ?? "").split(@VAR_delimiter@).filter(@VAR_value@ => @VAR_value@ && @VAR_value@ !== @VAR_shim@)].join(@VAR_delimiter@);
	}};
	{output_capture}
	const @VAR_restoreEnvironment@ = (@VAR_name@, @VAR_value@) => @VAR_value@ === undefined ? delete process.env[@VAR_name@] : process.env[@VAR_name@] = @VAR_value@;
	const @VAR_finishCommand@ = () => {{
	    const @VAR_command@ = @VAR_activeCommand@;
	    if (!@VAR_command@) return;
	    @VAR_activeCommand@ = undefined;
	    @VAR_server@.writer = @VAR_command@.previousWriter;
	    @VAR_command@.capture.restore();
	    @VAR_writeFileSync@(@VAR_join@(@VAR_command@.output, "{stdout_file}"), Buffer.concat(@VAR_command@.capture.stdout));
	    @VAR_writeFileSync@(@VAR_join@(@VAR_command@.output, "{stderr_file}"), Buffer.concat(@VAR_command@.capture.stderr));
	    const @VAR_elapsed@ = `${{Math.max(1, Math.ceil(performance.now() - @VAR_command@.started))}}ms`;
	    @VAR_runHelper@(["internal-write-done", "--command-id", @VAR_command@.id, "--exit-code", @VAR_command@.failed ? "1" : "0", "--time-consumption", @VAR_elapsed@, "--cwd", process.cwd(), "--directory", @VAR_command@.directory]);
	    @VAR_restoreEnvironment@("{command_id_env}", @VAR_command@.previousId);
	    @VAR_restoreEnvironment@("{command_directory_env}", @VAR_command@.previousDirectory);
	}};
	const @VAR_displayPrompt@ = @VAR_server@.displayPrompt.bind(@VAR_server@);
	@VAR_server@.displayPrompt = (...@VAR_args@) => {{
	    const @VAR_nativeError@ = @VAR_activeCommand@ && @VAR_server@.context._error !== @VAR_activeCommand@.previousError;
	    if (@VAR_nativeError@) {{
	        @VAR_activeCommand@.failed = true;
	        @VAR_pendingEvaluations@ = 0;
	        @VAR_evaluationReturned@ = true;
	    }}
	    if (@VAR_activeCommand@ && (@VAR_inputDepth@ > 0 || @VAR_pendingEvaluations@ > 0 || !@VAR_evaluationReturned@)) {{
	        @VAR_deferredPrompt@ = @VAR_args@;
	        return;
	    }}
	    @VAR_finishCommand@();
	    return @VAR_displayPrompt@(...@VAR_args@);
	}};
	const @VAR_finishDeferredPrompt@ = () => {{
	    if (!@VAR_activeCommand@ || @VAR_inputDepth@ > 0 || @VAR_pendingEvaluations@ > 0 || !@VAR_evaluationReturned@ || !@VAR_deferredPrompt@) return;
	    const @VAR_args@ = @VAR_deferredPrompt@;
	    @VAR_deferredPrompt@ = undefined;
	    @VAR_finishCommand@();
	    @VAR_displayPrompt@(...@VAR_args@);
	}};
	@VAR_server@.eval = (@VAR_source@, @VAR_context@, @VAR_file@, @VAR_callback@) => {{
	    if (@VAR_activeCommand@) @VAR_activeCommand@.readingInput = false;
	    @VAR_pendingEvaluations@ += 1;
	    let @VAR_callbackCalled@ = false;
	    const @VAR_finishEvaluation@ = (@VAR_error@, @VAR_value@) => {{
	        if (@VAR_callbackCalled@) return;
	        @VAR_callbackCalled@ = true;
	        if (@VAR_activeCommand@ && @VAR_error@ instanceof @VAR_Recoverable@ && @VAR_inputDepth@ > 0) {{
	            @VAR_activeCommand@.readingInput = true;
	        }} else if (@VAR_activeCommand@ && @VAR_error@) {{
	            @VAR_activeCommand@.failed = true;
	        }}
	        @VAR_pendingEvaluations@ -= 1;
	        @VAR_callback@(@VAR_error@, @VAR_value@);
	        if (@VAR_activeCommand@ && @VAR_inputDepth@ > 0) @VAR_activeCommand@.readingInput = true;
	        @VAR_finishDeferredPrompt@();
	    }};
	    try {{
	        const @VAR_returned@ = @VAR_evaluate@.call(@VAR_server@, @VAR_source@, @VAR_context@, @VAR_file@, @VAR_finishEvaluation@);
	        if (@VAR_returned@ && typeof @VAR_returned@.then === "function") {{
	            @VAR_returned@.then(@VAR_value@ => @VAR_finishEvaluation@(null, @VAR_value@), @VAR_error@ => @VAR_finishEvaluation@(@VAR_error@));
	        }}
	    }} finally {{
	        @VAR_evaluationReturned@ = true;
	        @VAR_finishDeferredPrompt@();
	    }}
	}};
	@VAR_server@.on("exit", () => {{
	    @VAR_finishCommand@();
	    @VAR_watcher@.close();
	}});
	const @VAR_stateDirectory@ = @VAR_join@(process.env.FUNCTERM_SESSION_ROOT, "{session_state}");
	const @VAR_dispatchFile@ = @VAR_join@(@VAR_stateDirectory@, "{dispatch_file}");
	const @VAR_dispatch@ = () => {{
	    if (@VAR_dispatching@ || @VAR_activeCommand@) return;
	    @VAR_dispatching@ = true;
	    try {{
	        const @VAR_commandId@ = @VAR_readFileSync@(@VAR_dispatchFile@, "utf8");
	        @VAR_rmSync@(@VAR_dispatchFile@);
	        const @VAR_directory@ = @VAR_join@(process.env.FUNCTERM_SESSION_ROOT, "{commands_directory}", @VAR_commandId@);
	        const @VAR_input@ = @VAR_join@(@VAR_directory@, "{input_directory}");
	        const @VAR_output@ = @VAR_join@(@VAR_directory@, "{output_directory}");
	        const @VAR_source@ = @VAR_readFileSync@(@VAR_join@(@VAR_input@, "{command_file}"), "utf8");
	        const @VAR_workingDirectory@ = @VAR_readFileSync@(@VAR_join@(@VAR_input@, "{working_directory_file}"), "utf8");
	        const @VAR_previousId@ = process.env.{command_id_env};
	        const @VAR_previousDirectory@ = process.env.{command_directory_env};
	        const @VAR_previousError@ = @VAR_server@.context._error;
	        process.env.{command_id_env} = @VAR_commandId@;
	        process.env.{command_directory_env} = @VAR_directory@;
	        @VAR_runHelper@(["internal-ensure-shims", "--directory", process.env.FUNCTERM_SHIM_DIR]);
	        @VAR_prependShimPath@();
	        process.chdir(@VAR_workingDirectory@);
	        @VAR_runHelper@(["internal-write-start", "--command-id", @VAR_commandId@, "--directory", @VAR_directory@]);
	        const @VAR_previousWriter@ = @VAR_server@.writer;
	        @VAR_server@.writer = @VAR_value@ => @VAR_inspect@(@VAR_value@, {{ ...@VAR_previousWriter@.options, colors: false }});
	        @VAR_activeCommand@ = {{
	            id: @VAR_commandId@, directory: @VAR_directory@, output: @VAR_output@,
	            previousId: @VAR_previousId@, previousDirectory: @VAR_previousDirectory@,
	            previousError: @VAR_previousError@, previousWriter: @VAR_previousWriter@,
	            started: performance.now(), failed: false, readingInput: true,
	            nativeInput: @VAR_source@.trimStart().startsWith("."), capture: @VAR_captureOutput@()
	        }};
	        @VAR_evaluationReturned@ = true;
	        @VAR_inputDepth@ += 1;
	        try {{
	            @VAR_server@.write(@VAR_source@.endsWith("\n") ? @VAR_source@ : `${{@VAR_source@}}\n`);
	        }} finally {{
	            @VAR_inputDepth@ -= 1;
	        }}
	        @VAR_finishDeferredPrompt@();
	        @VAR_server@.resume();
	    }} catch (@VAR_error@) {{
	        console.error(@VAR_error@);
	        @VAR_server@.close();
	    }} finally {{
	        @VAR_dispatching@ = false;
	    }}
	}};
	const @VAR_watcher@ = @VAR_watch@(@VAR_stateDirectory@, () => {{
	    if (!@VAR_existsSync@(@VAR_dispatchFile@)) return;
	    try {{
	        @VAR_dispatch@();
	    }} catch (@VAR_error@) {{
	        console.error(@VAR_error@);
	        @VAR_server@.close();
	    }}
	}});
	@VAR_server@.resume();
	@VAR_writeFileSync@({ready}, "");
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
        output_capture = output_capture::SCRIPT,
    );
    crate::shell::wrappers::VariableNamespace::new().render(&bootstrap)
}
