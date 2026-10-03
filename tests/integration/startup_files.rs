use crate::support::{
    create_tab, locked_with_env, parse_command_result, parse_tab_created, run_cli, send_command,
    temp_dir,
};
use std::fs;
#[test]
fn bash_preserves_user_profile_and_repairs_dispatch_conflicts() {
    let home = tempfile::tempdir().unwrap();
    fs::write(
        home.path().join(".bashrc"),
        r#"export PROFILE_MARKER=PROFILE_LOADED
profile_function() { printf '%s\n' "$PROFILE_MARKER"; }
alias profile_alias='profile_function'
alias f='printf BROKEN_DISPATCH'
export FUNCTERM_SESSION_ROOT=broken
export FUNCTERM_HELPER_EXECUTABLE=broken
export FUNCTERM_LOAD_PROFILE=0
PS1='CUSTOM_PROFILE_PROMPT> '
HISTFILE="$HOME/.bash_history"
HISTSIZE=1000
set -o history
"#,
    )
    .unwrap();
    let _guard = locked_with_env(&[("HOME", home.path().to_str().unwrap())]);
    let cwd = temp_dir("bash-profile");
    let created = create_tab(&cwd, "bash");
    let loaded = parse_command_result(&send_command(
        &created.tab_id,
        "profile_alias\nprintf '%s\\n' \"$FUNCTERM_LOAD_PROFILE\"\nbuiltin test -z \"${HISTFILE-}\" || exit 99\nfalse",
        10.0,
    ));
    assert!(loaded.finished, "{}", loaded.stderr);
    assert_eq!(loaded.stdout, "PROFILE_LOADED\n1\n");
    assert_eq!(loaded.exit_code, Some(1_i32));
    let viewed = run_cli(&["view", &created.tab_id]);
    assert!(
        String::from_utf8(viewed.stdout)
            .unwrap()
            .contains("CUSTOM_PROFILE_PROMPT")
    );
    let disabled = run_cli(&[
        "new-tab",
        "--starting-directory",
        cwd.to_str().unwrap(),
        "--starting-shell",
        "bash",
        "--no-profile",
    ]);
    assert!(disabled.status.success(), "{disabled:?}");
    let tab = parse_tab_created(&disabled);
    let clean = parse_command_result(&send_command(
        &tab.tab_id,
        "printf '%s\\n' \"${PROFILE_MARKER-unset}\" \"$FUNCTERM_LOAD_PROFILE\"",
        10.0,
    ));
    assert_eq!(clean.exit_code, Some(0_i32));
    assert_eq!(clean.stdout, "unset\n0\n");
}
#[test]
fn zsh_preserves_zdotdir_and_loads_startup_files_once() {
    let home = tempfile::tempdir().unwrap();
    let relocated = home.path().join("relocated");
    fs::create_dir(&relocated).unwrap();
    fs::write(
        home.path().join(".zshenv"),
        "export ZDOTDIR=\"$HOME/relocated\"\nexport PROFILE_LOADS=env\n",
    )
    .unwrap();
    fs::write(
        relocated.join(".zshrc"),
        r#"export PROFILE_LOADS="${PROFILE_LOADS}:rc"
profile_function() { print -r -- "$PROFILE_LOADS"; }
alias f='print BROKEN_DISPATCH'
export FUNCTERM_SESSION_ROOT=broken
"#,
    )
    .unwrap();
    let _guard = locked_with_env(&[("HOME", home.path().to_str().unwrap())]);
    let created = create_tab(&temp_dir("zsh-profile"), "zsh");
    let result = parse_command_result(&send_command(
        &created.tab_id,
        "profile_function\nprint -r -- \"$ZDOTDIR\"",
        10.0,
    ));
    assert!(result.finished, "{}", result.stderr);
    assert_eq!(result.exit_code, Some(0_i32), "{}", result.stderr);
    assert_eq!(result.stdout, format!("env:rc\n{}\n", relocated.display()));
}
