pub(super) const SCRIPT: &str = r#"class @VAR_InteractiveConsole@(@VAR_console@.InteractiveConsole):
    def runcode(self, @VAR_code@):
        self.@VAR_status@ = 0
        try:
            exec(@VAR_code@, self.locals, self.locals)
        except SystemExit:
            raise
        except BaseException as @VAR_error@:
            self.@VAR_status@ = 130 if isinstance(@VAR_error@, KeyboardInterrupt) else 1
            self.showtraceback()
    def execute(self, @VAR_source@):
        try:
            @VAR_flags@ = self.compile.compiler.flags | @VAR_ast@.PyCF_ONLY_AST
            @VAR_tree@ = compile(@VAR_source@, "<console>", "exec", @VAR_flags@, True)
            @VAR_code@ = self.compile.compiler(
                @VAR_ast@.Interactive(@VAR_tree@.body), "<console>", "single"
            )
        except (SyntaxError, OverflowError, ValueError):
            self.showsyntaxerror()
            return 1
        self.runcode(@VAR_code@)
        return self.@VAR_status@
@VAR_interpreter@ = @VAR_InteractiveConsole@(globals())
"#;
