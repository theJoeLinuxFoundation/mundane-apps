use super::*;
use std::io::{self, Write};

#[derive(Clone, Copy, Default, PartialEq)]
pub(super) enum Indent {
    #[default]
    Tabs,
    Two,
    Four,
    Eight,
}
impl Indent {
    fn choices() -> [Self; 4] {
        [Self::Tabs, Self::Two, Self::Four, Self::Eight]
    }
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Tabs => "Tabs",
            Self::Two => "2 spaces",
            Self::Four => "4 spaces",
            Self::Eight => "8 spaces",
        }
    }
    fn unit(self) -> &'static str {
        match self {
            Self::Tabs => "\t",
            Self::Two => "  ",
            Self::Four => "    ",
            Self::Eight => "        ",
        }
    }
    fn path() -> io::Result<PathBuf> {
        Ok(dirs::config_dir()
            .ok_or(io::ErrorKind::NotFound)?
            .join("mundane-text-editor/indentation"))
    }
    pub(super) fn load() -> io::Result<Self> {
        match std::fs::read_to_string(Self::path()?) {
            Ok(s) => Self::choices()
                .into_iter()
                .find(|i| i.label() == s.trim())
                .ok_or_else(|| io::Error::other("Unknown indentation setting.")),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::Tabs),
            Err(e) => Err(e),
        }
    }
    pub(super) fn next(self) -> io::Result<Self> {
        let choices = Self::choices();
        let next = choices[(choices.iter().position(|&v| v == self).unwrap() + 1) % choices.len()];
        let path = Self::path()?;
        let dir = path.parent().unwrap();
        std::fs::create_dir_all(dir)?;
        let mut tmp = tempfile::NamedTempFile::new_in(dir)?;
        writeln!(tmp, "{}", next.label())?;
        tmp.persist(path)?;
        Ok(next)
    }
}
impl Editor {
    pub(super) fn indent_key(
        &mut self,
        k: &Keystroke,
        w: &mut Window,
        cx: &mut Context<Self>,
    ) -> bool {
        if !matches!(k.key.as_str(), "tab" | "enter")
            || self.busy.is_some()
            || self.confirmation.is_some()
            || k.modifiers.control
            || k.modifiers.alt
            || k.modifiers.platform
        {
            return false;
        }
        let field = self.page().body.clone();
        let f = field.read(cx);
        if !f.focus_handle(cx).is_focused(w) {
            return false;
        }
        let sel = f.selection();
        let text = f.text();
        let start = text[..sel.start].rfind('\n').map_or(0, |i| i + 1);
        let prefix = &text[start..sel.start];
        let unit = self.indent.unit();
        let replacement = match k.key.as_str() {
            "enter" => {
                let leading = prefix
                    .chars()
                    .take_while(|&c| c == ' ' || c == '\t')
                    .collect::<String>();
                if leading.is_empty() {
                    return false;
                }
                format!("\n{leading}")
            }
            "tab" if !k.modifiers.shift && sel.is_empty() => {
                if unit == "\t" {
                    unit.to_string()
                } else {
                    let col = prefix.chars().fold(0, |col, c| {
                        if c == '\t' {
                            col + unit.len() - col % unit.len()
                        } else {
                            col + 1
                        }
                    });
                    " ".repeat(unit.len() - col % unit.len())
                }
            }
            "tab" => {
                let end = if sel.end > sel.start && text[..sel.end].ends_with('\n') {
                    sel.end
                } else {
                    text[sel.end..]
                        .find('\n')
                        .map_or(text.len(), |i| sel.end + i + 1)
                };
                let updated = text[start..end]
                    .split_inclusive('\n')
                    .map(|line| {
                        if k.modifiers.shift {
                            if let Some(tail) = line.strip_prefix('\t') {
                                tail.to_owned()
                            } else {
                                let n = line
                                    .bytes()
                                    .take(if unit == "\t" { 4 } else { unit.len() })
                                    .take_while(|&b| b == b' ')
                                    .count();
                                line[n..].to_owned()
                            }
                        } else {
                            format!("{unit}{line}")
                        }
                    })
                    .collect::<String>();
                if updated == text[start..end] {
                    return false;
                }
                let cursor = sel
                    .start
                    .saturating_sub((end - start).saturating_sub(updated.len()))
                    .max(start);
                let selected = if sel.is_empty() {
                    cursor..cursor
                } else {
                    start..start + updated.len()
                };
                field.update(cx, |f, cx| {
                    f.select_match(start..end, cx);
                    f.replace_document_selection(&updated, cx);
                    f.select_match(selected, cx)
                });
                return true;
            }
            _ => return false,
        };
        field.update(cx, |f, cx| {
            f.replace_text_in_range(None, &replacement, w, cx)
        });
        true
    }
}
