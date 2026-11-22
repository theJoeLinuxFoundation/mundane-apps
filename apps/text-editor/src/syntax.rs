use mundane_ui::SyntaxRole;
use std::{ops::Range, path::Path, sync::OnceLock};
use tree_sitter_highlight::{HighlightConfiguration, HighlightEvent, Highlighter};

pub fn language(path: Option<&Path>) -> Option<&'static str> {
    match path?.extension()?.to_str()? {
        "rs" => Some("Rust"),
        "py" | "pyw" => Some("Python"),
        "json" => Some("JSON"),
        _ => None,
    }
}
pub fn spans(language: &str, source: &str) -> Vec<(Range<usize>, SyntaxRole)> {
    static RUST: OnceLock<HighlightConfiguration> = OnceLock::new();
    static PYTHON: OnceLock<HighlightConfiguration> = OnceLock::new();
    static JSON: OnceLock<HighlightConfiguration> = OnceLock::new();
    let (cache, grammar, query) = match language {
        "Rust" => (
            &RUST,
            tree_sitter_rust::LANGUAGE,
            tree_sitter_rust::HIGHLIGHTS_QUERY,
        ),
        "Python" => (
            &PYTHON,
            tree_sitter_python::LANGUAGE,
            tree_sitter_python::HIGHLIGHTS_QUERY,
        ),
        "JSON" => (
            &JSON,
            tree_sitter_json::LANGUAGE,
            tree_sitter_json::HIGHLIGHTS_QUERY,
        ),
        _ => return vec![],
    };
    let config = cache.get_or_init(|| {
        let mut config = HighlightConfiguration::new(grammar.into(), language, query, "", "")
            .expect("pinned highlight query");
        config.configure(&[
            "keyword", "string", "comment", "number", "constant", "function", "type", "property",
        ]);
        config
    });
    let mut engine = Highlighter::new();
    let mut active = vec![];
    let mut spans = vec![];
    if let Ok(events) = engine.highlight(config, source.as_bytes(), None, |_| None) {
        for event in events.flatten() {
            match event {
                HighlightEvent::HighlightStart(h) => active.push(h.0),
                HighlightEvent::HighlightEnd => {
                    active.pop();
                }
                HighlightEvent::Source { start, end } => {
                    if let Some(i) = active.last() {
                        let role = match i {
                            0 => SyntaxRole::Keyword,
                            1 => SyntaxRole::String,
                            2 => SyntaxRole::Comment,
                            3 | 4 => SyntaxRole::Literal,
                            _ => SyntaxRole::Name,
                        };
                        spans.push((start..end, role));
                    }
                }
            }
        }
    }
    spans
}
