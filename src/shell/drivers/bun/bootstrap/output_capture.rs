pub(super) const SCRIPT: &str = r#"const @VAR_captureOutput@ = () => {
	    const @VAR_stdout@ = [], @VAR_stderr@ = [];
	    const @VAR_stdoutWrite@ = process.stdout.write.bind(process.stdout);
	    const @VAR_stderrWrite@ = process.stderr.write.bind(process.stderr);
	    const @VAR_write@ = (@VAR_original@, @VAR_chunks@, @VAR_isStdout@) => (@VAR_chunk@, ...@VAR_args@) => {
	        const @VAR_bytes@ = Buffer.isBuffer(@VAR_chunk@) ? @VAR_chunk@ : Buffer.from(@VAR_chunk@, typeof @VAR_args@[0] === "string" ? @VAR_args@[0] : undefined);
	        let @VAR_captured@ = @VAR_bytes@;
	        if (@VAR_isStdout@ && @VAR_activeCommand@?.readingInput) {
	            @VAR_captured@ = undefined;
	            if (@VAR_activeCommand@.nativeInput) {
	                const @VAR_lineEnd@ = @VAR_bytes@.indexOf(10);
	                if (@VAR_lineEnd@ >= 0) {
	                    @VAR_activeCommand@.readingInput = false;
	                    @VAR_captured@ = @VAR_bytes@.subarray(@VAR_lineEnd@ + 1);
	                }
	            }
	        }
	        if (@VAR_captured@?.length) @VAR_chunks@.push(@VAR_captured@);
	        return @VAR_original@(@VAR_chunk@, ...@VAR_args@);
	    };
	    process.stdout.write = @VAR_write@(@VAR_stdoutWrite@, @VAR_stdout@, true);
	    process.stderr.write = @VAR_write@(@VAR_stderrWrite@, @VAR_stderr@, false);
	    const @VAR_consoleMethods@ = [];
	    for (const @VAR_consoleObject@ of new Set([console, @VAR_server@.context.console])) {
	        for (const [@VAR_name@, @VAR_target@] of [["log", process.stdout], ["info", process.stdout], ["warn", process.stderr], ["error", process.stderr]]) {
	            @VAR_consoleMethods@.push([@VAR_consoleObject@, @VAR_name@, @VAR_consoleObject@[@VAR_name@]]);
	            @VAR_consoleObject@[@VAR_name@] = (...@VAR_values@) => @VAR_target@.write(`${@VAR_format@(...@VAR_values@)}\n`);
	        }
	    }
	    return {
	        stdout: @VAR_stdout@, stderr: @VAR_stderr@,
	        restore() {
	            process.stdout.write = @VAR_stdoutWrite@;
	            process.stderr.write = @VAR_stderrWrite@;
	            for (const [@VAR_consoleObject@, @VAR_name@, @VAR_method@] of @VAR_consoleMethods@) @VAR_consoleObject@[@VAR_name@] = @VAR_method@;
	        }
    };
};"#;
