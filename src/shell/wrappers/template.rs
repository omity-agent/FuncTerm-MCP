use crate::contract::{
    COMMAND_DIRECTORY_ENV, COMMAND_FILE, COMMAND_ID_ENV, COMMAND_INPUT_DIRECTORY,
    COMMAND_OUTPUT_DIRECTORY, COMMAND_SCRIPT_FILE, COMMAND_STATE_DIRECTORY, DONE_FILE,
    HELPER_EXECUTABLE_ENV, STDERR_FILE, STDOUT_FILE,
};
const COMMON: [(&str, &str); 9] = [
    ("@COMMAND_DIR_ENV@", COMMAND_DIRECTORY_ENV),
    ("@COMMAND_ID_ENV@", COMMAND_ID_ENV),
    ("@INPUT_DIR@", COMMAND_INPUT_DIRECTORY),
    ("@DONE@", DONE_FILE),
    ("@HELPER_ENV@", HELPER_EXECUTABLE_ENV),
    ("@OUTPUT_DIR@", COMMAND_OUTPUT_DIRECTORY),
    ("@STATE_DIR@", COMMAND_STATE_DIRECTORY),
    ("@STDERR@", STDERR_FILE),
    ("@STDOUT@", STDOUT_FILE),
];
pub(super) fn render_command_function(template: &str, function_name: &str) -> String {
    render(
        template,
        &[("@FUNCTION@", function_name), ("@COMMAND@", COMMAND_FILE)],
    )
}
pub(super) fn render_script(template: &str) -> String {
    render(template, &[("@SCRIPT@", COMMAND_SCRIPT_FILE)])
}
fn render(template: &str, extra: &[(&str, &str)]) -> String {
    let mut text = template.to_owned();
    for &(placeholder, value) in COMMON.iter().chain(extra.iter()) {
        text = text.replace(placeholder, value);
    }
    text
}
