pub(super) const FUNCTIONS: &str = r#"def save_nushell_state [
    @VAR_cwd_file@: path,
    @VAR_env_state_file@: path,
    @VAR_config_state_file@: path,
    @VAR_declaration_state_file@: path,
] {
    if not ($env.FUNCTERM_SHIM_DIR? | is-empty) {
        $env.PATH = ($env.PATH | where {|@VAR_entry@| $@VAR_entry@ != $env.FUNCTERM_SHIM_DIR } | prepend $env.FUNCTERM_SHIM_DIR)
    }
    $env.PWD | save --force --raw $@VAR_cwd_file@
    let @VAR_environment_entries@ = $env
        | reject --optional PWD FILE_PWD CURRENT_FILE config ENV_CONVERSIONS @NUSHELL_PROTECTED_ENVIRONMENT@
        | transpose @VAR_name@ @VAR_value@
        | where {|@VAR_item@| not (($@VAR_item@.@VAR_value@ | describe) starts-with 'closure') }
    let @VAR_saved_environment@ = if ($@VAR_environment_entries@ | is-empty) {
        {}
    } else {
        $@VAR_environment_entries@ | transpose --header-row --as-record
    }
    $@VAR_saved_environment@
        | to nuon
        | save --force --raw $@VAR_env_state_file@
    let @VAR_saved_config@ = $env.config? | default {}
    $@VAR_saved_config@ | reject --optional hooks completions keybindings menus
        | to nuon
        | save --force --raw $@VAR_config_state_file@
    let @VAR_declarations@ = scope commands
        | where type == custom
        | where name not-in ['banner' 'pwd' 'exit' 'save_nushell_state' 'f' 'functerm_run_command' 'ensure_nushell_shims' 'write_nushell_config' 'restore_nushell_environment']
        | uniq-by name
        | each {|@VAR_item@| view source $@VAR_item@.name }
    let @VAR_aliases@ = scope aliases
        | where name != 'f'
        | each {|@VAR_item@| $@VAR_item@ | format pattern "alias {name} = {expansion}" }
    let @VAR_source@ = $@VAR_declarations@ | append $@VAR_aliases@
    if not ($@VAR_source@ | is-empty) {
        $@VAR_source@ | str join (char newline)
            | save --force --raw $@VAR_declaration_state_file@
    }
}"#;
