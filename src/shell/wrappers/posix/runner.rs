use super::super::start;
use super::PosixDialect;
use crate::contract::{COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV, HELPER_EXECUTABLE_ENV};
pub(super) fn helpers(dialect: PosixDialect) -> String {
    format!(
        r#"functerm_restore_command_environment() {{
{emulate}    if {test_one}; then
        export {command_id_env}="$2"
    else
        unset {command_id_env}
    fi
    if {test_three}; then
        export {command_dir_env}="$4"
    else
        unset {command_dir_env}
    fi
}}
functerm_publish_done() {{
{emulate}    local @VAR_command_id@="$1"
    local @VAR_exit_code@="$2"
    local @VAR_time_consumption@="$3"
    local @VAR_cwd@="$4"
    local @VAR_native_directory@="$5"
    local @VAR_helper@="${{{helper_env}-}}"
    if [ -z "$@VAR_helper@" ]; then
        printf '%s is not set\n' "{helper_env}" >&2
        return 1
    fi
    @VAR_helper@="$(functerm_posix_path "$@VAR_helper@")" || return 1
    if command -v cygpath > /dev/null 2>&1; then
        @VAR_cwd@="$(cygpath -m "$@VAR_cwd@")" || return 1
    fi
    "$@VAR_helper@" internal-write-done \
        --command-id "$@VAR_command_id@" \
        --exit-code "$@VAR_exit_code@" \
        --time-consumption "$@VAR_time_consumption@" \
        --cwd "$@VAR_cwd@" \
        --directory "$@VAR_native_directory@"
}}
{publish_start}
functerm_command_time_millis() {{
{emulate}    local @VAR_helper@="${{{helper_env}-}}"
    if [ -z "$@VAR_helper@" ]; then
        printf '%s is not set\n' "{helper_env}" >&2
        return 1
    fi
    @VAR_helper@="$(functerm_posix_path "$@VAR_helper@")" || return 1
    "$@VAR_helper@" internal-time-millis
}}
functerm_restore_status() {{ return "$1"; }}"#,
        emulate = dialect.emulate(),
        helper_env = HELPER_EXECUTABLE_ENV,
        command_id_env = COMMAND_ID_ENV,
        command_dir_env = COMMAND_DIRECTORY_ENV,
        test_one = dialect.test_arg("1"),
        test_three = dialect.test_arg("3"),
        publish_start = start::posix(dialect),
    )
}
