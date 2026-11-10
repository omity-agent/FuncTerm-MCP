pub(super) const SOURCE: &str = r#"public sealed class FuncTermConsoleHost : PSHost {
    readonly PSHost owner;
    readonly PSHostUserInterface ui;
    readonly Action<int> exiting;
    public FuncTermConsoleHost(PSHost host, bool separate, Action<int> exiting) {
        var property = host.GetType().GetProperty(
            "ExternalHost", BindingFlags.Instance | BindingFlags.NonPublic);
        if (property == null) throw new InvalidOperationException("PowerShell host has no external host");
        owner = (PSHost)property.GetValue(host);
        ui = separate ? new DiagnosticUi(owner.UI) : owner.UI;
        this.exiting = exiting;
    }
    public override string Name { get { return owner.Name; } }
    public override Version Version { get { return owner.Version; } }
    public override Guid InstanceId { get { return owner.InstanceId; } }
    public override CultureInfo CurrentCulture { get { return owner.CurrentCulture; } }
    public override CultureInfo CurrentUICulture { get { return owner.CurrentUICulture; } }
    public override PSHostUserInterface UI { get { return ui; } }
    public override PSObject PrivateData { get { return owner.PrivateData; } }
    public override void SetShouldExit(int code) { exiting(code); owner.SetShouldExit(code); }
    public override void EnterNestedPrompt() { owner.EnterNestedPrompt(); }
    public override void ExitNestedPrompt() { owner.ExitNestedPrompt(); }
    public override void NotifyBeginApplication() { owner.NotifyBeginApplication(); }
    public override void NotifyEndApplication() { owner.NotifyEndApplication(); }
    public static void Separate(Pipeline pipeline) {
        var errors = pipeline.Commands[pipeline.Commands.Count - 1];
        pipeline.Commands.RemoveAt(pipeline.Commands.Count - 1);
        pipeline.Commands[0].MergeMyResults(PipelineResultTypes.None, PipelineResultTypes.None);
        var formatter = new Command("Microsoft.PowerShell.Utility\\Out-String");
        formatter.Parameters.Add("Stream", true);
        pipeline.Commands.Add(formatter);
        var writer = new Command("Microsoft.PowerShell.Core\\ForEach-Object");
        writer.Parameters.Add("Process", ScriptBlock.Create("[Console]::WriteLine($_)"));
        pipeline.Commands.Add(writer);
        pipeline.Commands.Add(errors);
    }
    sealed class DiagnosticUi : PSHostUserInterface {
        readonly PSHostUserInterface original;
        public DiagnosticUi(PSHostUserInterface ui) { original = ui; }
        public override PSHostRawUserInterface RawUI { get { return original.RawUI; } }
        public override bool SupportsVirtualTerminal { get { return original.SupportsVirtualTerminal; } }
        public override string ReadLine() { return original.ReadLine(); }
        public override SecureString ReadLineAsSecureString() { return original.ReadLineAsSecureString(); }
        public override void Write(string value) { Console.Out.Write(value); }
        public override void Write(ConsoleColor foreground, ConsoleColor background, string value) { Write(value); }
        public override void WriteLine() { Console.Out.WriteLine(); }
        public override void WriteLine(string value) { Console.Out.WriteLine(value); }
        public override void WriteLine(ConsoleColor foreground, ConsoleColor background, string value) { WriteLine(value); }
        public override void WriteErrorLine(string value) { Console.Error.WriteLine(value); }
        public override void WriteWarningLine(string value) { Console.Error.WriteLine("WARNING: " + value); }
        public override void WriteVerboseLine(string value) { Console.Error.WriteLine("VERBOSE: " + value); }
        public override void WriteDebugLine(string value) { Console.Error.WriteLine("DEBUG: " + value); }
        public override void WriteProgress(long source, ProgressRecord record) { original.WriteProgress(source, record); }
        public override Dictionary<string, PSObject> Prompt(string caption, string message, Collection<FieldDescription> fields) {
            if (!string.IsNullOrEmpty(caption)) Console.Out.WriteLine(caption);
            if (!string.IsNullOrEmpty(message)) Console.Out.WriteLine(message);
            foreach (var field in fields) Console.Out.Write(field.Name + ": ");
            Console.Out.Flush();
            return original.Prompt(caption, message, fields);
        }
        public override int PromptForChoice(string caption, string message, Collection<ChoiceDescription> choices, int selected) {
            return original.PromptForChoice(caption, message, choices, selected);
        }
        public override PSCredential PromptForCredential(string caption, string message, string user, string target) {
            return original.PromptForCredential(caption, message, user, target);
        }
        public override PSCredential PromptForCredential(string caption, string message, string user, string target, PSCredentialTypes types, PSCredentialUIOptions options) {
            return original.PromptForCredential(caption, message, user, target, types, options);
        }
    }
}
"#;
