use alacritty_terminal::{
    index::{Direction, Point},
    term::{
        Term,
        search::{RegexIter, RegexSearch},
    },
};

pub fn web_url(text: &str) -> Option<String> {
    let text = text.trim();
    if text.chars().any(|c| c.is_whitespace() || c.is_control()) {
        return None;
    }
    let parsed = url::Url::parse(text).ok()?;
    (matches!(parsed.scheme(), "http" | "https") && parsed.host().is_some()).then(|| parsed.into())
}
pub fn at<T>(screen: &Term<T>, point: Point) -> Option<String> {
    if let Some(link) = screen.grid()[point].hyperlink() {
        return web_url(link.uri());
    }
    let mut re = RegexSearch::new(r#"https?://[^\s<>"']+"#).ok()?;
    let mut hits = RegexIter::new(
        screen.line_search_left(point),
        screen.line_search_right(point),
        Direction::Right,
        screen,
        &mut re,
    );
    let span = hits.find(|r| r.contains(&point))?;
    let raw = screen.bounds_to_string(*span.start(), *span.end());
    let mut text = raw.trim_end_matches(['.', ',', ';', ':', '!', '?']);
    while text.ends_with(')') && text.matches(')').count() > text.matches('(').count() {
        text = &text[..text.len() - 1];
    }
    web_url(text)
}
