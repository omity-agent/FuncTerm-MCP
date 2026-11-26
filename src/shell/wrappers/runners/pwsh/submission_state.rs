pub(super) const SOURCE: &str = r#"public sealed class FuncTermNativeWrite : ArgumentTransformationAttribute {
    public bool Written;
    public override object Transform(EngineIntrinsics engine, object value) {
        Written = true;
        return value;
    }
}
public sealed class FuncTermSubmission {
    const BindingFlags PrivateInstance = BindingFlags.Instance | BindingFlags.NonPublic;
    readonly Runspace runspace;
    readonly PSHost host;
    readonly EngineIntrinsics engine;
    readonly string[] protectedNames;
    readonly FuncTermHelper helper;
    readonly MethodInfo runningPipeline, setHost, resetHost;
    readonly PropertyInfo availability;
    readonly Stopwatch timer = new Stopwatch();
    readonly FuncTermNativeWrite nativeWrite = new FuncTermNativeWrite();
    FuncTermEnvironment environment;
    FuncTermOutputCapture capture;
    Pipeline execution;
    PSVariable nativeVariable;
    string commandId, directory, previousId, previousDirectory;
    bool separate, hostAttached, failed, createdNativeVariable;
    public FuncTermSubmission(Runspace runspace, PSHost host, EngineIntrinsics engine, string[] protectedNames) {
        this.runspace = runspace;
        this.host = host;
        this.engine = engine;
        this.protectedNames = protectedNames;
        helper = new FuncTermHelper(Environment.GetEnvironmentVariable("@HELPER_ENV@"),
            Environment.GetEnvironmentVariable("FUNCTERM_SESSION_ROOT"));
        runningPipeline = runspace.GetType().GetMethod("GetCurrentlyRunningPipeline", PrivateInstance);
        setHost = host.GetType().GetMethod("SetHostRef", PrivateInstance);
        resetHost = host.GetType().GetMethod("RevertHostRef", PrivateInstance);
        availability = runspace.GetType().GetProperty("RunspaceAvailability");
        if (runningPipeline == null || setHost == null || resetHost == null || availability == null)
            throw new InvalidOperationException("PowerShell host does not expose submission lifecycle hooks");
        runspace.AvailabilityChanged += BeginExecution;
        FuncTermEnvironment.EnsureShims();
    }
    public string Claim() { return helper.Claim(); }
    public string Accept(string id, string path, bool separateOutput) {
        var source = File.ReadAllText(Path.Combine(path, "@SCRIPT@"), System.Text.Encoding.UTF8);
        commandId = id;
        directory = path;
        separate = separateOutput;
        hostAttached = failed = false;
        environment = new FuncTermEnvironment(protectedNames);
        previousId = Environment.GetEnvironmentVariable("@COMMAND_ID_ENV@");
        previousDirectory = Environment.GetEnvironmentVariable("@COMMAND_DIR_ENV@");
        Environment.SetEnvironmentVariable("@COMMAND_ID_ENV@", id);
        Environment.SetEnvironmentVariable("@COMMAND_DIR_ENV@", path);
        FuncTermEnvironment.EnsureShims();
        nativeVariable = engine.SessionState.PSVariable.Get("global:LASTEXITCODE");
        createdNativeVariable = nativeVariable == null;
        if (createdNativeVariable) {
            engine.SessionState.PSVariable.Set("global:LASTEXITCODE", null);
            nativeVariable = engine.SessionState.PSVariable.Get("global:LASTEXITCODE");
        }
        nativeVariable.Attributes.Add(nativeWrite);
        nativeWrite.Written = false;
        helper.Invoke("internal-write-start", "--command-id", id, "--directory", path);
        timer.Restart();
        CurrentPipeline().StateChanged += ReleaseInput;
        if (string.IsNullOrWhiteSpace(source)) Complete(0, false);
        else engine.InvokeCommand.PreCommandLookupAction += ObservePrompt;
        return source;
    }
    Pipeline CurrentPipeline() { return (Pipeline)runningPipeline.Invoke(runspace, null); }
    static bool IsInteractive(Pipeline pipeline) {
        return (bool)pipeline.GetType().GetProperty("AddToHistory", PrivateInstance).GetValue(pipeline);
    }
    static bool HasEnded(PipelineState state) {
        return state == PipelineState.Completed || state == PipelineState.Failed || state == PipelineState.Stopped;
    }
    void ReleaseInput(object sender, PipelineStateEventArgs args) {
        if (!HasEnded(args.PipelineStateInfo.State)) return;
        ((Pipeline)sender).StateChanged -= ReleaseInput;
        if (args.PipelineStateInfo.State == PipelineState.Completed &&
            CurrentPipeline() == null && runspace.RunspaceAvailability == RunspaceAvailability.Busy)
            availability.SetValue(runspace, RunspaceAvailability.Available);
    }
    void BeginExecution(object sender, RunspaceAvailabilityEventArgs args) {
        if (commandId == null || execution != null || args.RunspaceAvailability != RunspaceAvailability.Busy) return;
        try {
            var pipeline = CurrentPipeline();
            if (pipeline == null || pipeline.Commands.Count == 0 || !IsInteractive(pipeline)) return;
            execution = pipeline;
            if (separate) {
                capture = new FuncTermOutputCapture(directory);
                FuncTermConsoleHost.Separate(pipeline);
            }
            setHost.Invoke(host, new object[] { new FuncTermConsoleHost(host, separate, Exit) });
            hostAttached = true;
        } catch (Exception error) {
            failed = true;
            ReportFailure("prepare", error);
        }
    }
    void ObservePrompt(object sender, CommandLookupEventArgs args) {
        if (args.CommandName != "prompt" || (execution != null && !HasEnded(execution.PipelineStateInfo.State))) return;
        try {
            var pipeline = CurrentPipeline();
            if (pipeline != null && pipeline.Commands.Count == 1 &&
                pipeline.Commands[0].CommandText == "prompt" && !IsInteractive(pipeline))
                Complete(null, false);
        } catch (Exception error) {
            AbortCompletion(error);
        }
    }
    static void ReportFailure(string action, Exception error) {
        Console.Error.WriteLine("FuncTerm could not " + action + " the PowerShell submission.");
        if ((Environment.GetEnvironmentVariable("FUNCTERM_DIAGNOSTICS_FILTER") ?? "").Contains("debug"))
            Console.Error.WriteLine(error);
    }
    static void AbortCompletion(Exception error) {
        try { ReportFailure("complete", error); }
        finally { Environment.Exit(1); }
    }
    void Exit(int code) { Complete(code, true); }
    void Complete(int? explicitCode, bool sessionExit) {
        if (commandId == null) return;
        engine.InvokeCommand.PreCommandLookupAction -= ObservePrompt;
        try {
            PublishCompletion(explicitCode, sessionExit);
        } catch (Exception error) {
            AbortCompletion(error);
        }
    }
    void PublishCompletion(int? explicitCode, bool sessionExit) {
        timer.Stop();
        var state = execution?.PipelineStateInfo.State;
        var succeeded = !failed && state != PipelineState.Failed &&
            LanguagePrimitives.IsTrue(engine.SessionState.PSVariable.GetValue("?"));
        var nativeCode = nativeVariable.Value;
        var exitCode = explicitCode ?? (state == PipelineState.Stopped ? 130 : succeeded ? 0 :
            nativeWrite.Written && nativeCode != null && LanguagePrimitives.ConvertTo<int>(nativeCode) != 0
                ? LanguagePrimitives.ConvertTo<int>(nativeCode) : 1);
        nativeVariable.Attributes.Remove(nativeWrite);
        if (createdNativeVariable && !nativeWrite.Written) engine.SessionState.PSVariable.Remove("global:LASTEXITCODE");
        if (hostAttached) resetHost.Invoke(host, null);
        if (capture != null) {
            capture.Dispose();
            Console.Write(File.ReadAllText(capture.Stdout));
            Console.Error.Write(File.ReadAllText(capture.Stderr));
            capture = null;
        }
        environment.Restore();
        FuncTermEnvironment.EnsureShims();
        var elapsed = timer.Elapsed.TotalMilliseconds.ToString(CultureInfo.InvariantCulture) + "ms";
        helper.Invoke("internal-write-done", "--command-id", commandId,
            "--exit-code", exitCode.ToString(CultureInfo.InvariantCulture), "--time-consumption", elapsed,
            "--cwd", engine.SessionState.Path.CurrentFileSystemLocation.ProviderPath, "--directory", directory,
            "--session-exit", sessionExit ? "true" : "false");
        Environment.SetEnvironmentVariable("@COMMAND_ID_ENV@", previousId);
        Environment.SetEnvironmentVariable("@COMMAND_DIR_ENV@", previousDirectory);
        commandId = null;
        execution = null;
    }
}
"#;
