use regex::Regex;
use std::sync::LazyLock;
use tsugiki::dom::NodeRef;

pub fn strip_tags(node: &NodeRef) -> String {
    let mut accum = String::new();
    for node in node.inclusive_descendants() {
        if let Some(text) = node.as_text() {
            accum.push_str(text.borrow().content.as_str());
        }
    }
    accum
}

pub fn inner_text(node: &NodeRef) -> String {
    static REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new("\\s+").unwrap());

    let mut accum = String::new();
    let mut trailing_space_emitted = false;
    for node in node.inclusive_descendants() {
        if let Some(text) = node.as_text() {
            let text = text.borrow();
            let text = text.content.as_str();

            let processed = REGEX.replace_all(text, " ");
            let start_stripped = processed.trim_start();
            let has_start = processed != start_stripped;
            let end_stripped = start_stripped.trim_end();
            let has_ending = text != end_stripped;

            if end_stripped.is_empty() {
                if !trailing_space_emitted && (has_start || has_ending) {
                    accum.push(' ');
                    trailing_space_emitted = true;
                }
            } else {
                if !trailing_space_emitted && has_start {
                    accum.push(' ');
                }
                accum.push_str(&end_stripped);
                if has_ending {
                    accum.push(' ');
                }
                trailing_space_emitted = has_ending;
            }
        }
    }
    accum
}
