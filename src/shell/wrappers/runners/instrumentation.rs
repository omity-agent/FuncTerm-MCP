pub(super) const SCRIPT: &str = r"Add-Type -TypeDefinition @'
using System;
using System.Linq;
using System.Management.Automation;
using System.Management.Automation.Language;
public static class FuncTermScript {
    static UsingStatementAst CopyUsing(UsingStatementAst statement) {
        var name = statement.Name == null ? null : (StringConstantExpressionAst)statement.Name.Copy();
        if (statement.ModuleSpecification != null) {
            var specification = (HashtableAst)statement.ModuleSpecification.Copy();
            return name == null ? new UsingStatementAst(statement.Extent, specification)
                : new UsingStatementAst(statement.Extent, name, specification);
        }
        return statement.Alias == null
            ? new UsingStatementAst(statement.Extent, statement.UsingStatementKind, name)
            : new UsingStatementAst(statement.Extent, statement.UsingStatementKind, name,
                (StringConstantExpressionAst)statement.Alias.Copy());
    }
    static ScriptBlockAst Parse(string source) {
        Token[] tokens;
        ParseError[] errors;
        var tree = Parser.ParseInput(source, out tokens, out errors);
        if (errors.Length != 0) throw new ParseException(errors);
        return tree;
    }
    static NamedBlockAst Wrap(NamedBlockAst block, StatementBlockAst finalizer) {
        if (block == null) return null;
        var statements = new StatementBlockAst(block.Extent,
            block.Statements.Select(statement => (StatementAst)statement.Copy()),
            block.Traps == null ? null : block.Traps.Select(trap => (TrapStatementAst)trap.Copy()));
        var guarded = new TryStatementAst(block.Extent, statements, null,
            (StatementBlockAst)finalizer.Copy());
        return new NamedBlockAst(block.Extent, block.BlockKind,
            new StatementBlockAst(block.Extent, new[] { guarded }, null), block.Unnamed);
    }
    public static ScriptBlock Create(string source, string cleanup, string sentinel) {
        var original = Parse(source);
        var finish = Parse(cleanup);
        var finalizer = new StatementBlockAst(finish.Extent,
            finish.EndBlock.Statements.Select(statement => (StatementAst)statement.Copy()), null);
        var body = new ScriptBlockAst(original.Extent, null,
            Wrap(original.BeginBlock, finalizer), Wrap(original.ProcessBlock, finalizer),
            Wrap(original.EndBlock, finalizer), Wrap(original.CleanBlock, finalizer),
            Wrap(original.DynamicParamBlock, finalizer));
        var expression = new ScriptBlockExpressionAst(original.Extent, body);
        var invocation = new CommandAst(original.Extent, new[] { expression }, TokenKind.Dot, null);
        var returned = Parse(sentinel).EndBlock.Statements.Select(statement => (StatementAst)statement.Copy());
        var outer = new StatementBlockAst(original.Extent,
            new StatementAst[] { new PipelineAst(original.Extent, invocation) }.Concat(returned), null);
        return new ScriptBlockAst(original.Extent,
            original.UsingStatements.Select(CopyUsing),
            original.Attributes.Select(attribute => (AttributeAst)attribute.Copy()),
            original.ParamBlock == null ? null : (ParamBlockAst)original.ParamBlock.Copy(),
            outer, false, false).GetScriptBlock();
    }
}
public sealed class FuncTermNativeWrite : ArgumentTransformationAttribute {
    public bool Written;
    public override object Transform(EngineIntrinsics engine, object value) {
        Written = true;
        return value;
    }
}
'@
function Format-FuncTermError {
    param([System.Management.Automation.ErrorRecord]$@VAR_record@)
    $@VAR_record@ | Microsoft.PowerShell.Utility\Out-String
}
";
