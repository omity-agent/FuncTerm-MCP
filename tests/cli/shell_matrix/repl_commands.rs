use super::matrix::{ShellCase, case_dir, required_executable};
use crate::support::{create_tab, locked_with_env, parse_command_result, send_command};
use std::fs;
const BUN: ShellCase = ShellCase {
    name: "bun",
    env_var: "FUNCTERM_BUN",
    executables: &["bun", "bun.exe"],
    expected_exit_code: 0,
};
#[test]
fn bun_native_repl_commands_use_original_input_pipeline() {
    let executable = required_executable(&BUN);
    let _guard = locked_with_env(&[(BUN.env_var, &executable)]);
    let cwd = case_dir(BUN.name, "native repl commands");
    let loaded_file = "functerm-repl-load.js";
    let saved_file = "functerm-repl-save.js";
    fs :: write (cwd . join (loaded_file) , "globalThis.FuncTermLoadedMarker = 'MCP_PTY_BUN_LOADED';\nglobalThis.FuncTermSecondLoadedMarker = 'MCP_PTY_BUN_SECOND_LOADED';\n" ,) . unwrap () ;
    let created = create_tab(&cwd, BUN.name);
    let defined = parse_command_result(&send_command(
        &created.tab_id,
        "globalThis.FuncTermSavedMarker = 'MCP_PTY_BUN_SAVED'",
        10.0,
    ));
    assert_eq!(defined.exit_code, Some(0_i32));
    let help = parse_command_result(&send_command(&created.tab_id, ".help", 10.0));
    for command in [".break", ".clear", ".exit", ".help", ".load", ".save"] {
        assert!(
            help.stdout.contains(command),
            "Bun help should list {command}: {}",
            help.stdout
        );
    }
    let saved = parse_command_result(&send_command(
        &created.tab_id,
        &format!(".save {saved_file}"),
        10.0,
    ));
    assert!(saved.stdout.contains("Session saved"));
    let saved_source = fs::read_to_string(cwd.join(saved_file)).unwrap();
    assert!(
        saved_source.contains("globalThis.FuncTermSavedMarker = 'MCP_PTY_BUN_SAVED'"),
        "saved REPL source should contain original user input: {saved_source}"
    );
    assert!(
        !saved_source.contains("await f()"),
        "saved REPL source must not contain FuncTerm dispatch input: {saved_source}"
    );
    let cleared = parse_command_result(&send_command(&created.tab_id, ".clear", 10.0));
    assert!(cleared.stdout.contains("Clearing context"));
    let missing = parse_command_result(&send_command(
        &created.tab_id,
        "typeof FuncTermSavedMarker",
        10.0,
    ));
    assert!(missing.stdout.contains("'undefined'"));
    let loaded = parse_command_result(&send_command(
        &created.tab_id,
        &format!(".load {loaded_file}"),
        10.0,
    ));
    assert_eq!(loaded.exit_code, Some(0_i32));
    let queried =
        parse_command_result(&send_command(&created.tab_id, "FuncTermLoadedMarker", 10.0));
    assert!(queried.stdout.contains("MCP_PTY_BUN_LOADED"));
    let second = parse_command_result(&send_command(
        &created.tab_id,
        "FuncTermSecondLoadedMarker",
        10.0,
    ));
    assert!(second.stdout.contains("MCP_PTY_BUN_SECOND_LOADED"));
    let multiline = parse_command_result(&send_command(
        &created.tab_id,
        "globalThis.FuncTermFirstLine = 'MCP_PTY_BUN_FIRST_LINE'\nglobalThis.FuncTermSecondLine = 'MCP_PTY_BUN_SECOND_LINE'",
        10.0,
    ));
    assert!(multiline.stdout.contains("MCP_PTY_BUN_FIRST_LINE"));
    assert!(multiline.stdout.contains("MCP_PTY_BUN_SECOND_LINE"));
    let failed = parse_command_result(&send_command(
        &created.tab_id,
        "throw new Error('MCP_PTY_BUN_NATIVE_ERROR')",
        10.0,
    ));
    assert_eq!(failed.exit_code, Some(1_i32));
    assert!(failed.stdout.contains("MCP_PTY_BUN_NATIVE_ERROR"));
    let broken = parse_command_result(&send_command(&created.tab_id, ".break", 10.0));
    assert!(broken.finished);
    assert_eq!(broken.exit_code, Some(0_i32));
}
