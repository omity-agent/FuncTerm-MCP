use crate::shell::shims::SHIM_DIR_ENV;
pub(super) fn path_function(zsh: bool) -> String {
    let local_options = if zsh {
        "\n    emulate -L zsh\n    setopt sh_word_split"
    } else {
        ""
    };
    format!(
        r#"functerm_posix_path() {{{local_options}
    local value="$1"
    if command -v cygpath > /dev/null 2>&1; then
        cygpath -u "$value"
        return $?
    fi
    case "$value" in
        [A-Za-z]:\\*|[A-Za-z]:/*)
            printf 'cygpath is required to convert Windows path: %s\n' "$value" >&2
            return 1
            ;;
    esac
    printf '%s\n' "$value"
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
    local shim_dir="${{{SHIM_DIR_ENV}}}"
    shim_dir="$(functerm_posix_path "$shim_dir")" || return 1
    local new_path="$shim_dir"
    local old_ifs="$IFS"
    local entry
    IFS=:
    for entry in $PATH; do
        if [ "$entry" != "$shim_dir" ]; then
            new_path="$new_path:$entry"
        fi
    done
    IFS="$old_ifs"
    export PATH="$new_path"
}}
functerm_prepend_shim_path"#
    )
}
