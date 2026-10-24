pub(super) const SCRIPT: &str = r#"const @VAR_renderInput@ = @VAR_server@.output.write.bind(@VAR_server@.output);
@VAR_server@.output = new Proxy(@VAR_server@.output, {
    get(@VAR_target@, @VAR_property@) {
        const @VAR_value@ = Reflect.get(@VAR_target@, @VAR_property@, @VAR_target@);
        if (@VAR_property@ !== "write") return typeof @VAR_value@ === "function" ? @VAR_value@.bind(@VAR_target@) : @VAR_value@;
        return (@VAR_chunk@, ...@VAR_args@) => {
            if (@VAR_activeCommand@?.readingInput) {
                if (@VAR_inputDepth@ === 0) return @VAR_renderInput@(@VAR_chunk@, ...@VAR_args@);
                const @VAR_callback@ = @VAR_args@.at(-1);
                if (typeof @VAR_callback@ === "function") @VAR_callback@();
                return true;
            }
            return @VAR_value@.call(@VAR_target@, @VAR_chunk@, ...@VAR_args@);
        };
    }
});
const @VAR_forwardInput@ = @VAR_server@.write.bind(@VAR_server@);
@VAR_server@.write = (...@VAR_args@) => {
    @VAR_inputDepth@ += 1;
    if (@VAR_activeCommand@) @VAR_activeCommand@.readingInput = true;
    try {
        return @VAR_forwardInput@(...@VAR_args@);
    } finally {
        @VAR_inputDepth@ -= 1;
    }
};
@VAR_server@.prependListener("line", () => {
    if (@VAR_activeCommand@) @VAR_activeCommand@.readingInput = false;
});
@VAR_server@.on("line", () => {
    if (@VAR_activeCommand@) @VAR_activeCommand@.readingInput = true;
});"#;
