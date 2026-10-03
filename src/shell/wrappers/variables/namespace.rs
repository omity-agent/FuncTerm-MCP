use itertools::Itertools as _;
const SUFFIX_LENGTH: usize = 12;
const BASE36_ALPHABET: [char; 36] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f', 'g', 'h', 'i',
    'j', 'k', 'l', 'm', 'n', 'o', 'p', 'q', 'r', 's', 't', 'u', 'v', 'w', 'x', 'y', 'z',
];
const MARKER_START: &str = "@VAR_";
pub(in crate::shell) struct VariableNamespace {
    suffix: String,
}
impl VariableNamespace {
    pub(in crate::shell) fn new() -> Self {
        Self {
            suffix: nanoid::nanoid!(SUFFIX_LENGTH, &BASE36_ALPHABET),
        }
    }
    pub(in crate::shell) fn render(&self, template: &str) -> String {
        let mut rendered = String::with_capacity(template.len());
        let mut remaining = template;
        while let Some((before_marker, marker_tail)) = remaining.split_once(MARKER_START) {
            rendered.push_str(before_marker);
            let (semantic, after_marker) = split_marker(marker_tail);
            rendered.push_str(semantic);
            rendered.push('_');
            rendered.push_str(&self.suffix);
            remaining = after_marker;
        }
        rendered.push_str(remaining);
        rendered
    }
    pub(in crate::shell) fn render_powershell(&self, template: &str) -> String {
        let names = template
            .split(MARKER_START)
            .skip(1)
            .map(|tail| split_marker(tail).0)
            .unique()
            .format_with(", ", |name, format| {
                format(&format_args!("'{name}_{}'", self.suffix))
            });
        let cleanup = format!(
            "Get-Variable -Scope Local | Where-Object Name -In @({names}) | Remove-Variable -Scope Local"
        );
        let script = template.replace("@POWERSHELL_STATE_CLEANUP@", &cleanup);
        self.render(&script)
    }
}
fn split_marker(tail: &str) -> (&str, &str) {
    let Some((semantic, remaining)) = tail.split_once('@') else {
        panic!("wrapper variable marker must end with '@'");
    };
    assert!(
        valid_semantic_prefix(semantic),
        "wrapper variable semantic prefix is invalid: {semantic}"
    );
    (semantic, remaining)
}
fn valid_semantic_prefix(value: &str) -> bool {
    value
        .bytes()
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic() || first == b'_')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
