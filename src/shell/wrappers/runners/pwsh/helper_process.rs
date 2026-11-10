pub(super) const SOURCE: &str = r#"public sealed class FuncTermHelper {
    readonly string executable, root;
    public FuncTermHelper(string executable, string root) {
        if (string.IsNullOrEmpty(executable)) throw new InvalidOperationException("PowerShell helper is not configured");
        this.executable = executable;
        this.root = root;
    }
    public string Claim() {
        return Invoke("internal-claim-command", "--session-root", root, "--shell", "powershell");
    }
    public string Invoke(params string[] arguments) {
        var info = new ProcessStartInfo(executable) {
            UseShellExecute = false, RedirectStandardOutput = true, RedirectStandardError = true,
            StandardOutputEncoding = System.Text.Encoding.UTF8, StandardErrorEncoding = System.Text.Encoding.UTF8
        };
        foreach (var argument in arguments) info.ArgumentList.Add(argument);
        using (var process = Process.Start(info)) {
            if (process == null) throw new InvalidOperationException("PowerShell helper did not start");
            var output = process.StandardOutput.ReadToEndAsync();
            var error = process.StandardError.ReadToEndAsync();
            process.WaitForExit();
            var stdout = output.GetAwaiter().GetResult();
            var stderr = error.GetAwaiter().GetResult();
            if (process.ExitCode != 0) throw new InvalidOperationException(
                "PowerShell helper failed: " + arguments[0] + ": " + stderr);
            if (stderr.Length != 0) Console.Error.Write(stderr);
            return stdout;
        }
    }
}
public sealed class FuncTermEnvironment {
    readonly System.Collections.IDictionary snapshot = Environment.GetEnvironmentVariables();
    readonly string[] protectedNames;
    public FuncTermEnvironment(string[] protectedNames) { this.protectedNames = protectedNames; }
    public void Restore() {
        var cleared = Environment.GetEnvironmentVariables().Count == 0;
        foreach (System.Collections.DictionaryEntry entry in snapshot) {
            var name = (string)entry.Key;
            if (cleared || protectedNames.Contains(name, StringComparer.OrdinalIgnoreCase))
                Environment.SetEnvironmentVariable(name, (string)entry.Value);
        }
    }
    public static void EnsureShims() {
        var shim = Environment.GetEnvironmentVariable("FUNCTERM_SHIM_DIR");
        if (string.IsNullOrEmpty(shim)) return;
        var entries = (Environment.GetEnvironmentVariable("PATH") ?? string.Empty).Split(Path.PathSeparator);
        Environment.SetEnvironmentVariable("PATH", string.Join(Path.PathSeparator.ToString(),
            new[] { shim }.Concat(entries.Where(entry => !string.Equals(entry, shim, StringComparison.OrdinalIgnoreCase)))));
    }
}
"#;
