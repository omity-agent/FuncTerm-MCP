use super::PosixDialect;
use crate::contract::{COMMAND_DIRECTORY_ENV, COMMAND_ID_ENV};
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
    local @VAR_replay@=''
    if [ "${{@VAR_separate_output@-0}}" = 1 ]; then
        @VAR_replay@=--replay-output
    fi
    @VAR_cwd@="$(functerm_native_cwd)" || return 1
    functerm_terminal_helper "$@VAR_helper_path@" internal-write-done \
        --command-id "$@VAR_command_id@" \
        --exit-code "$@VAR_exit_code@" \
        --time-consumption "$@VAR_time_consumption@" \
        --cwd "$@VAR_cwd@" \
        --directory "$@VAR_native_directory@" $@VAR_replay@
}}
{publish_start}
functerm_command_time_millis() {{
{emulate}    local @VAR_millis@
    @VAR_millis@="$("$@VAR_helper_path@" internal-time-millis < /dev/null 2>&1)" || {{
        builtin printf '%s\n' "$@VAR_millis@" >&2
        return 1
    }}
    builtin printf '%s' "$@VAR_millis@"
}}
functerm_terminal_helper() (
    builtin set -o pipefail
    "$@" --stdio < /dev/null 2>&1 | "$@VAR_cat@" > /dev/tty
)
functerm_restore_status() {{ return "$1"; }}"#,
        emulate = dialect.emulate(),
        command_id_env = COMMAND_ID_ENV,
        command_dir_env = COMMAND_DIRECTORY_ENV,
        test_one = dialect.test_arg("1"),
        test_three = dialect.test_arg("3"),
        publish_start = publish_start(dialect),
    )
}
fn publish_start(dialect: PosixDialect) -> String {
    format!(
        r#"functerm_publish_start() {{
{emulate}    local @VAR_command_id@="$1"
            local @VAR_native_directory@="$2"
            functerm_terminal_helper "$@VAR_helper_path@" internal-write-start \
                --command-id "$@VAR_command_id@" \
                --directory "$@VAR_native_directory@"
        }}"#,
        emulate = dialect.emulate(),
    )
}
