use super::posix_dialect::PosixDialect;
use crate::shell::shims::SHIM_DIR_ENV;
pub(in crate::shell) fn bash_wrapper() -> String {
    let wrapper = format!(
        "set +o history
	unset HISTFILE
export HISTSIZE=0
export HISTFILESIZE=0
history -c
	{path}
	{shim_path}
	{command}
	{dispatcher}
	",
        path = path_function(false),
        shim_path = shim_path_function(false),
        command = super::posix_function::command_function(PosixDialect::Bash),
        dispatcher = super::template::posix_dispatcher()
    );
    super::VariableNamespace::new().render(&wrapper)
}
pub(in crate::shell) fn zsh_wrapper() -> String {
    let wrapper = format!(
        "unset HISTFILE
	HISTSIZE=0
	SAVEHIST=0
	setopt no_append_history
	setopt no_share_history
	setopt no_inc_append_history
	fc -p /dev/null 0 0 2> /dev/null || true
	unset HISTFILE
	{path}
	{shim_path}
	{command}
	{dispatcher}
	",
        path = path_function(true),
        shim_path = shim_path_function(true),
        command = super::posix_function::command_function(PosixDialect::Zsh),
        dispatcher = super::template::posix_dispatcher()
    );
    super::VariableNamespace::new().render(&wrapper)
}
pub(super) fn path_function(zsh: bool) -> String {
    let local_options = if zsh {
        "\n    emulate -L zsh\n    setopt sh_word_split"
    } else {
        ""
    };
    format!(
        r#"functerm_posix_path() {{{local_options}
	    local @VAR_value@="$1"
	    if command -v cygpath > /dev/null 2>&1; then
	        cygpath -u "$@VAR_value@"
	        return $?
	    fi
	    case "$@VAR_value@" in
	        [A-Za-z]:\\*|[A-Za-z]:/*)
	            printf 'cygpath is required to convert Windows path: %s\n' "$@VAR_value@" >&2
	            return 1
	            ;;
	    esac
	    printf '%s\n' "$@VAR_value@"
}}"#
    )
}
pub(super) fn shim_path_function(zsh: bool) -> String {
    let local_options = if zsh {
        "\n    emulate -L zsh\n    setopt sh_word_split"
    } else {
        ""
    };
    format!(
        r#"functerm_prepend_shim_path() {{{local_options}
    if [ -z "${{{SHIM_DIR_ENV}-}}" ]; then
        return 0
    fi
	    local @VAR_shim_dir@="${{{SHIM_DIR_ENV}}}"
	    @VAR_shim_dir@="$(functerm_posix_path "$@VAR_shim_dir@")" || return 1
	    local @VAR_new_path@="$@VAR_shim_dir@"
	    local @VAR_old_ifs@="$IFS"
	    local @VAR_entry@
	    IFS=:
	    for @VAR_entry@ in $PATH; do
	        if [ "$@VAR_entry@" != "$@VAR_shim_dir@" ]; then
	            @VAR_new_path@="$@VAR_new_path@:$@VAR_entry@"
	        fi
	    done
	    IFS="$@VAR_old_ifs@"
	    export PATH="$@VAR_new_path@"
}}
functerm_prepend_shim_path"#
    )
}
