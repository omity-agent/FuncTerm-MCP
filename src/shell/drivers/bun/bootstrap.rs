mod echo_control;
mod transport;
use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, DISPATCH_FILE, HELPER_EXECUTABLE_ENV,
    SESSION_EXIT_FILE, SESSION_STATE_DIRECTORY,
};
pub(super) const fn host_script() -> &'static str {
    transport::HOST
}
pub(super) fn script(cwd: &str, ready: &str) -> String {
    let bootstrap = format!(
        r#"import {{ spawnSync as @VAR_spawnSync@ }} from "node:child_process";
			import {{ existsSync as @VAR_existsSync@, readFileSync as @VAR_readFileSync@, watch as @VAR_watch@, writeFileSync as @VAR_writeFileSync@, writeSync as @VAR_writeSync@ }} from "node:fs";
	import {{ join as @VAR_join@, delimiter as @VAR_delimiter@ }} from "node:path";
	import {{ Recoverable as @VAR_Recoverable@, start as @VAR_start@ }} from "node:repl";
	import {{ format as @VAR_format@, inspect as @VAR_inspect@ }} from "node:util";
	process.chdir({cwd});
	process.env.FUNCTERM_CURRENT_SHELL = "bun";
		const @VAR_server@ = @VAR_start@({{ prompt: "> " }});
		@VAR_server@.context.console = console;
	const @VAR_evaluate@ = @VAR_server@.eval;
	let @VAR_activeCommand@;
	let @VAR_dispatching@ = false;
	let @VAR_inputDepth@ = 0;
	let @VAR_pendingEvaluations@ = 0;
		let @VAR_deferredPrompt@;
		let @VAR_evaluationReturned@ = false;
			{echo_control}
		const @VAR_runHelper@ = (@VAR_args@, @VAR_capture@ = false) => {{
	    const @VAR_helper@ = process.env.{helper_env};
	    if (!@VAR_helper@) throw new Error("{helper_env} is not set");
		    const @VAR_result@ = @VAR_spawnSync@(@VAR_helper@, @VAR_args@, {{ env: process.env, stdio: @VAR_capture@ ? "pipe" : "inherit", encoding: "utf8" }});
	    if (@VAR_result@.error) throw @VAR_result@.error;
		    if (@VAR_result@.status !== 0) throw new Error(`FuncTerm helper exited with ${{@VAR_result@.status}}: ${{@VAR_result@.stderr ?? ""}}`);
		    return @VAR_result@.stdout;
	}};
	const @VAR_prependShimPath@ = () => {{
	    const @VAR_shim@ = process.env.FUNCTERM_SHIM_DIR;
	    if (!@VAR_shim@) return;
	    process.env.PATH = [@VAR_shim@, ...(process.env.PATH ?? "").split(@VAR_delimiter@).filter(@VAR_value@ => @VAR_value@ && @VAR_value@ !== @VAR_shim@)].join(@VAR_delimiter@);
	}};
	{output_capture}
	const @VAR_restoreEnvironment@ = (@VAR_name@, @VAR_value@) => @VAR_value@ === undefined ? delete process.env[@VAR_name@] : process.env[@VAR_name@] = @VAR_value@;
		const @VAR_finishCommand@ = (@VAR_exitCode@ = undefined) => {{
	    const @VAR_command@ = @VAR_activeCommand@;
		    if (!@VAR_command@) return;
		    @VAR_activeCommand@ = undefined;
		    if (@VAR_exitCode@ !== undefined && !process.env.FUNCTERM_PARENT_SHELL) @VAR_writeFileSync@(@VAR_join@(@VAR_command@.directory, "{session_exit}"), "");
	    @VAR_server@.writer = @VAR_command@.previousWriter;
	    const @VAR_elapsed@ = `${{Math.max(1, Math.ceil(performance.now() - @VAR_command@.started))}}ms`;
	    const @VAR_environmentWasCleared@ = Object.keys(process.env).length === 0;
	    for (const [@VAR_name@, @VAR_value@] of Object.entries(@VAR_command@.protectedEnvironment)) {{
		        if (@VAR_environmentWasCleared@ || [@BUN_PROTECTED_ENVIRONMENT@].includes(@VAR_name@.toUpperCase())) {{
		            process.env[@VAR_name@] = @VAR_value@;
		        }}
		    }}
		    @VAR_prependShimPath@();
			    @VAR_streamBoundary@("end", @VAR_command@, {{ exitCode: @VAR_exitCode@ ?? (@VAR_command@.failed ? 1 : 0), elapsed: @VAR_elapsed@, cwd: process.cwd() }});
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
		        if (@VAR_activeCommand@) @VAR_activeCommand@.readingInput = false;
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
		    @VAR_watcher@.close();
		}});
		process.on("exit", @VAR_exitCode@ => @VAR_finishCommand@(@VAR_exitCode@));
	const @VAR_stateDirectory@ = @VAR_join@(process.env.FUNCTERM_SESSION_ROOT, "{session_state}");
	const @VAR_dispatchFile@ = @VAR_join@(@VAR_stateDirectory@, "{dispatch_file}");
	const @VAR_dispatch@ = () => {{
	    if (@VAR_dispatching@ || @VAR_activeCommand@) return;
	    @VAR_dispatching@ = true;
	    try {{
		        const @VAR_receipt@ = JSON.parse(@VAR_runHelper@(["internal-claim-command", "--session-root", process.env.FUNCTERM_SESSION_ROOT, "--shell", "bun"], true));
		        const @VAR_commandId@ = @VAR_receipt@.command_id;
		        const @VAR_directory@ = @VAR_receipt@.directory;
                const @VAR_source@ = @VAR_readFileSync@(@VAR_join@(@VAR_directory@, "{command_file}"), "utf8");
		        const @VAR_workingDirectory@ = @VAR_receipt@.working_directory;
	        const @VAR_previousId@ = process.env.{command_id_env};
	        const @VAR_previousDirectory@ = process.env.{command_directory_env};
	        const @VAR_protectedEnvironment@ = {{ ...process.env }};
	        const @VAR_previousError@ = @VAR_server@.context._error;
	        process.env.{command_id_env} = @VAR_commandId@;
	        process.env.{command_directory_env} = @VAR_directory@;
	        @VAR_prependShimPath@();
	        process.chdir(@VAR_workingDirectory@);
	        @VAR_runHelper@(["internal-write-start", "--command-id", @VAR_commandId@, "--directory", @VAR_directory@]);
	        const @VAR_previousWriter@ = @VAR_server@.writer;
	        if (@VAR_receipt@.separate_output) @VAR_server@.writer = @VAR_value@ => @VAR_inspect@(@VAR_value@, {{ ...@VAR_previousWriter@.options, colors: false }});
	        @VAR_activeCommand@ = {{
				id: @VAR_commandId@, directory: @VAR_directory@, output: @VAR_directory@,
	            previousId: @VAR_previousId@, previousDirectory: @VAR_previousDirectory@,
	            protectedEnvironment: @VAR_protectedEnvironment@,
	            previousError: @VAR_previousError@, previousWriter: @VAR_previousWriter@,
		            started: performance.now(), failed: false, readingInput: true,
			        }};
			        @VAR_streamBoundary@("start", @VAR_activeCommand@);
		        @VAR_evaluationReturned@ = true;
                    @VAR_inputDepth@ += 1;
                    try {{
                        @VAR_server@.emit("line", @VAR_source@);
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
        session_exit = SESSION_EXIT_FILE,
        dispatch_file = DISPATCH_FILE,
        command_file = COMMAND_FILE,
        command_id_env = COMMAND_ID_ENV,
        command_directory_env = COMMAND_DIRECTORY_ENV,
        output_capture = transport::BOUNDARY,
        echo_control = echo_control::SCRIPT,
    );
    let protected = bootstrap.replace(
        "@BUN_PROTECTED_ENVIRONMENT@",
        &crate::shell::wrappers::quoted_protected_environment_names(),
    );
    crate::shell::wrappers::VariableNamespace::new().render(&protected)
}
