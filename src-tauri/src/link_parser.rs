//! Extract `[[wiki links]]` and `#tags` from markdown text.

use regex::Regex;
use std::sync::OnceLock;

fn link_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\[\[([^\[\]]+)\]\]").unwrap())
}

fn tag_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // '#' at start of text or after whitespace, then a letter, then
    // letters/digits/_/-//. A markdown heading ("# Title") never matches
    // because '#' must be followed directly by a letter.
    RE.get_or_init(|| Regex::new(r"(?:^|\s)#([A-Za-z][A-Za-z0-9_/-]*)").unwrap())
}

/// `[[Note Name]]` -> "Note Name". Spaces inside the brackets are preserved,
/// surrounding whitespace is trimmed. Deduplicated, order of first appearance.
pub fn extract_links(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cap in link_re().captures_iter(content) {
        let target = cap[1].trim().to_string();
        if !target.is_empty() && !out.contains(&target) {
            out.push(target);
        }
    }
    out
}

/// `#tag` -> "tag" (lowercased). Deduplicated, order of first appearance.
pub fn extract_tags(content: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for cap in tag_re().captures_iter(content) {
        let tag = cap[1].to_lowercase();
        if !out.contains(&tag) {
            out.push(tag);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_with_spaces() {
        let links = extract_links("See [[Note Name]] and [[ Other Note ]] and [[Note Name]].");
        assert_eq!(
            links,
            vec!["Note Name".to_string(), "Other Note".to_string()]
        );
    }

    #[test]
    fn tags_but_not_headings() {
        let tags = extract_tags("# Heading\nSome #Ideas and #project/plinth here.");
        assert_eq!(
            tags,
            vec!["ideas".to_string(), "project/plinth".to_string()]
        );
    }
}
