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
    PSVariable nativeVariable;
    string commandId, directory, previousId, previousDirectory;
    bool separate, executing, stopped, failed, createdNativeVariable;
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
        runspace.AvailabilityChanged += ObservePipeline;
        FuncTermEnvironment.EnsureShims();
    }
    public string Claim() { return helper.Claim(); }
    public string Accept(string id, string path, bool separateOutput) {
        var source = File.ReadAllText(Path.Combine(path, "@SCRIPT@"), System.Text.Encoding.UTF8);
        commandId = id;
        directory = path;
        separate = separateOutput;
        executing = stopped = failed = false;
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
        ((Pipeline)runningPipeline.Invoke(runspace, null)).StateChanged += ReleaseInput;
        if (string.IsNullOrWhiteSpace(source)) Complete(0, false);
        return source;
    }
    void ReleaseInput(object sender, PipelineStateEventArgs args) {
        if (args.PipelineStateInfo.State == PipelineState.Completed &&
            runningPipeline.Invoke(runspace, null) == null && runspace.RunspaceAvailability == RunspaceAvailability.Busy)
            availability.SetValue(runspace, RunspaceAvailability.Available);
    }
    void ObservePipeline(object sender, RunspaceAvailabilityEventArgs args) {
        if (commandId == null || args.RunspaceAvailability != RunspaceAvailability.Busy) return;
        try {
            var pipeline = (Pipeline)runningPipeline.Invoke(runspace, null);
            if (pipeline == null || pipeline.Commands.Count == 0) return;
            var history = (bool)pipeline.GetType().GetProperty("AddToHistory", PrivateInstance).GetValue(pipeline);
            if ((Environment.GetEnvironmentVariable("FUNCTERM_DIAGNOSTICS_FILTER") ?? "").Contains("debug"))
                Console.Error.WriteLine("PowerShell submission " + commandId + ": executing=" + executing +
                    ", interactive=" + history + ", prompt=" + (pipeline.Commands[0].CommandText == "prompt"));
            if (!history && pipeline.Commands[0].CommandText == "prompt") {
                Complete(null, false);
            } else if (history && !executing) {
                executing = true;
                if (separate) {
                    capture = new FuncTermOutputCapture(directory);
                    FuncTermConsoleHost.Separate(pipeline);
                }
                setHost.Invoke(host, new object[] { new FuncTermConsoleHost(host, separate, Exit) });
                pipeline.StateChanged += ObserveCompletion;
            }
        } catch (Exception error) {
            failed = true;
            Console.Error.WriteLine("PowerShell submission lifecycle failed: " + error.Message);
        }
    }
    void ObserveCompletion(object sender, PipelineStateEventArgs args) {
        stopped = args.PipelineStateInfo.State == PipelineState.Stopped;
        failed |= args.PipelineStateInfo.State == PipelineState.Failed;
    }
    void Exit(int code) { Complete(code, true); }
    void Complete(int? explicitCode, bool sessionExit) {
        if (commandId == null) return;
        timer.Stop();
        var succeeded = LanguagePrimitives.IsTrue(engine.SessionState.PSVariable.GetValue("?"));
        var nativeCode = nativeVariable.Value;
        var exitCode = explicitCode ?? (stopped ? 130 : succeeded && !failed ? 0 :
            nativeWrite.Written && nativeCode != null && LanguagePrimitives.ConvertTo<int>(nativeCode) != 0
                ? LanguagePrimitives.ConvertTo<int>(nativeCode) : 1);
        nativeVariable.Attributes.Remove(nativeWrite);
        if (createdNativeVariable && !nativeWrite.Written) engine.SessionState.PSVariable.Remove("global:LASTEXITCODE");
        if (executing) resetHost.Invoke(host, null);
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
    }
}
"#;
