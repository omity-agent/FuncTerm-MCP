pub(super) const SCRIPT: &str = r#"if (-not ('FuncTermOutputCapture' -as [type])) {
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Runtime.InteropServices;
using System.Collections.Generic;
using System.Collections.ObjectModel;
using System.Globalization;
using System.Management.Automation;
using System.Management.Automation.Host;
using System.Reflection;
using System.Security;
public sealed class FuncTermDiagnosticHost : PSHost {
    readonly PSHost owner;
    readonly PSHostUserInterface ui;
    public FuncTermDiagnosticHost(PSHost host) {
        var property = host.GetType().GetProperty("ExternalHost", BindingFlags.Instance | BindingFlags.NonPublic);
        if (property == null) throw new InvalidOperationException("PowerShell invocation host has no external host");
        owner = (PSHost)property.GetValue(host);
        ui = new DiagnosticUi(owner.UI);
    }
    public override string Name { get { return owner.Name; } }
    public override Version Version { get { return owner.Version; } }
    public override Guid InstanceId { get { return owner.InstanceId; } }
    public override CultureInfo CurrentCulture { get { return owner.CurrentCulture; } }
    public override CultureInfo CurrentUICulture { get { return owner.CurrentUICulture; } }
    public override PSHostUserInterface UI { get { return ui; } }
    public override PSObject PrivateData { get { return owner.PrivateData; } }
    public override void SetShouldExit(int code) { owner.SetShouldExit(code); }
    public override void EnterNestedPrompt() { owner.EnterNestedPrompt(); }
    public override void ExitNestedPrompt() { owner.ExitNestedPrompt(); }
    public override void NotifyBeginApplication() { owner.NotifyBeginApplication(); }
    public override void NotifyEndApplication() { owner.NotifyEndApplication(); }
    sealed class DiagnosticUi : PSHostUserInterface {
        readonly PSHostUserInterface original;
        public DiagnosticUi(PSHostUserInterface ui) { original = ui; }
        public override PSHostRawUserInterface RawUI { get { return original.RawUI; } }
        public override bool SupportsVirtualTerminal { get { return original.SupportsVirtualTerminal; } }
        public override string ReadLine() { return original.ReadLine(); }
        public override SecureString ReadLineAsSecureString() { return original.ReadLineAsSecureString(); }
        public override void Write(string value) { Console.Out.Write(value); }
        public override void Write(ConsoleColor foreground, ConsoleColor background, string value) { Write(value); }
        public override void WriteLine(string value) { Console.Out.WriteLine(value); }
        public override void WriteErrorLine(string value) { Console.Error.WriteLine(value); }
        public override void WriteWarningLine(string value) { Console.Error.WriteLine("WARNING: " + value); }
        public override void WriteVerboseLine(string value) { Console.Error.WriteLine("VERBOSE: " + value); }
        public override void WriteDebugLine(string value) { Console.Error.WriteLine("DEBUG: " + value); }
        public override void WriteInformation(InformationRecord record) { Console.Out.WriteLine(record.MessageData); }
        public override void WriteProgress(long source, ProgressRecord record) { original.WriteProgress(source, record); }
        public override Dictionary<string, PSObject> Prompt(string caption, string message, Collection<FieldDescription> fields) {
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
public sealed class FuncTermOutputCapture : IDisposable {
    sealed class SharedFile : Stream {
        [DllImport("kernel32.dll", SetLastError=true)] static extern bool WriteFile(IntPtr handle, IntPtr buffer, uint count, out uint written, IntPtr overlapped);
        [DllImport("libc", SetLastError=true)] static extern IntPtr write(int fd, IntPtr buffer, UIntPtr count);
        readonly FileStream file;
        readonly IntPtr handle;
        readonly bool windows = Environment.OSVersion.Platform == PlatformID.Win32NT;
        public SharedFile(string path) {
            file = new FileStream(path, FileMode.Create, FileAccess.Write, FileShare.ReadWrite);
            handle = file.SafeFileHandle.DangerousGetHandle();
        }
        public IntPtr Handle { get { return handle; } }
        public override bool CanRead { get { return false; } }
        public override bool CanSeek { get { return false; } }
        public override bool CanWrite { get { return true; } }
        public override long Length { get { throw new NotSupportedException(); } }
        public override long Position {
            get { throw new NotSupportedException(); }
            set { throw new NotSupportedException(); }
        }
        public override void Flush() {}
        public override int Read(byte[] buffer, int offset, int count) { throw new NotSupportedException(); }
        public override long Seek(long offset, SeekOrigin origin) { throw new NotSupportedException(); }
        public override void SetLength(long value) { throw new NotSupportedException(); }
        public override void Write(byte[] buffer, int offset, int count) {
            var pinned = GCHandle.Alloc(buffer, GCHandleType.Pinned);
            try {
                while (count > 0) {
                    var pointer = IntPtr.Add(pinned.AddrOfPinnedObject(), offset);
                    int written;
                    if (windows) {
                        uint bytes;
                        Check(WriteFile(Handle, pointer, (uint)count, out bytes, IntPtr.Zero));
                        written = checked((int)bytes);
                    } else {
                        var bytes = write(Handle.ToInt32(), pointer, (UIntPtr)count).ToInt64();
                        Check(bytes >= 0);
                        written = checked((int)bytes);
                    }
                    if (written == 0) throw new IOException("Output capture write made no progress");
                    offset += written;
                    count -= written;
                }
            } finally { pinned.Free(); }
        }
        protected override void Dispose(bool disposing) {
            if (disposing) file.Dispose();
            base.Dispose(disposing);
        }
    }
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr GetStdHandle(int value);
    [DllImport("kernel32.dll", SetLastError=true)] static extern bool SetStdHandle(int value, IntPtr handle);
    [DllImport("libc", SetLastError=true)] static extern int dup(int fd);
    [DllImport("libc", SetLastError=true)] static extern int dup2(int source, int target);
    [DllImport("libc", SetLastError=true)] static extern int close(int fd);
    readonly TextWriter previousOut = Console.Out, previousError = Console.Error;
    readonly StreamWriter output, error;
    readonly IntPtr originalOutput, originalError;
    readonly int savedOutput, savedError;
    readonly bool windows = Environment.OSVersion.Platform == PlatformID.Win32NT;
    public string Stdout { get; private set; }
    public string Stderr { get; private set; }
    static void Check(bool succeeded) {
        if (!succeeded) throw new System.ComponentModel.Win32Exception(Marshal.GetLastWin32Error());
    }
    public FuncTermOutputCapture(string directory) {
        Stdout = Path.Combine(directory, "@STDOUT@");
        Stderr = Path.Combine(directory, "@STDERR@");
        previousOut.Flush(); previousError.Flush();
        output = new StreamWriter(new SharedFile(Stdout), new System.Text.UTF8Encoding(false));
        error = new StreamWriter(new SharedFile(Stderr), new System.Text.UTF8Encoding(false));
        output.AutoFlush = true; error.AutoFlush = true;
        if (windows) {
            originalOutput = GetStdHandle(-11); originalError = GetStdHandle(-12);
            Check(SetStdHandle(-11, ((SharedFile)output.BaseStream).Handle));
            Check(SetStdHandle(-12, ((SharedFile)error.BaseStream).Handle));
        } else {
            savedOutput = dup(1); savedError = dup(2);
            Check(savedOutput >= 0 && savedError >= 0);
            Check(dup2(((SharedFile)output.BaseStream).Handle.ToInt32(), 1) >= 0);
            Check(dup2(((SharedFile)error.BaseStream).Handle.ToInt32(), 2) >= 0);
        }
        Console.SetOut(output); Console.SetError(error);
    }
    public void Dispose() {
        output.Flush(); error.Flush();
        Console.SetOut(previousOut); Console.SetError(previousError);
        if (windows) {
            Check(SetStdHandle(-11, originalOutput)); Check(SetStdHandle(-12, originalError));
        } else {
            Check(dup2(savedOutput, 1) >= 0); Check(dup2(savedError, 2) >= 0);
            Check(close(savedOutput) == 0); Check(close(savedError) == 0);
        }
        output.Dispose(); error.Dispose();
    }
}
'@
}
function Complete-FuncTermOutputCapture {
    param($@VAR_capture@)
    $@VAR_capture@.Dispose()
    [Console]::Write([IO.File]::ReadAllText($@VAR_capture@.Stdout))
    [Console]::Error.Write([IO.File]::ReadAllText($@VAR_capture@.Stderr))
}
"#;
