use std::ffi::{c_char, c_void};
#[derive(Clone)]
pub struct Entry {
    pub glyph: String,
    group: usize,
    emoji: Option<&'static emojis::Emoji>,
    pub name: String,
}
impl Entry {
    pub fn code(&self) -> String {
        self.glyph
            .chars()
            .map(|c| format!("U+{:04X}", c as u32))
            .collect::<Vec<_>>()
            .join(" ")
    }
}
unsafe extern "C" {
    fn mundane_names(
        data: *mut c_void,
        add: extern "C" fn(*mut c_void, i32, *const c_char, i32),
    ) -> i32;
    fn mundane_character_group(code: i32) -> i32;
    fn mundane_extended_name(code: i32, name: *mut c_char, capacity: i32) -> i32;
}
extern "C" fn collect(context: *mut c_void, code: i32, name: *const c_char, len: i32) {
    if let Some(glyph) = char::from_u32(code as u32) {
        // ICU calls synchronously with a counted invariant-ASCII name.
        let bytes = unsafe { std::slice::from_raw_parts(name.cast(), len as usize) };
        let entries = unsafe { &mut *context.cast::<Vec<Entry>>() };
        entries.push(Entry {
            group: unsafe { mundane_character_group(code) } as usize,
            emoji: None,
            glyph: glyph.to_string(),
            name: String::from_utf8_lossy(bytes).into_owned(),
        })
    }
}
pub const GROUPS: [&str; 10] = [
    "All characters",
    "Letters",
    "Numbers",
    "Punctuation",
    "Symbols",
    "Marks",
    "Separators",
    "Other",
    "Emoji",
    "Recently copied",
];
pub struct Catalog(Vec<Entry>, usize);
impl Catalog {
    pub fn load() -> Result<Self, String> {
        let mut entries = Vec::<Entry>::new();
        if unsafe { mundane_names((&mut entries as *mut Vec<Entry>).cast(), collect) } == 0 {
            return Err("Could not read character names from ICU.".into());
        }
        let singles = entries.len();
        let mut sequences = std::collections::HashSet::new();
        for base in emojis::iter() {
            for emoji in base
                .skin_tones()
                .into_iter()
                .flatten()
                .chain(std::iter::once(base))
            {
                let glyph = emoji.as_str();
                if let Ok(i) = entries[..singles].binary_search_by(|e| e.glyph.as_str().cmp(glyph))
                {
                    entries[i].emoji = Some(emoji);
                } else if sequences.insert(glyph) {
                    entries.push(Entry {
                        glyph: glyph.into(),
                        name: emoji.name().to_uppercase(),
                        group: 4,
                        emoji: Some(emoji),
                    });
                }
            }
        }
        entries[singles..].sort_by(|a, b| a.glyph.cmp(&b.glyph));
        Ok(Self(entries, singles))
    }
    pub fn find(&self, query: &str, group: usize) -> Vec<usize> {
        let words: Vec<_> = query
            .split_ascii_whitespace()
            .map(str::to_ascii_uppercase)
            .collect();
        self.0
            .iter()
            .enumerate()
            .filter(|(_, e)| {
                (group == 0 || group == 8 && e.emoji.is_some() || group == e.group)
                    && words.iter().all(|w| {
                        e.name.contains(w)
                            || e.emoji.is_some_and(|emoji| {
                                emoji.name().to_uppercase().contains(w)
                                    || emoji.shortcodes().any(|s| s.to_uppercase().contains(w))
                            })
                    })
            })
            .map(|(i, _)| i)
            .collect()
    }
    pub fn get(&self, i: usize) -> &Entry {
        &self.0[i]
    }
    pub fn exact(&self, query: &str) -> Option<Result<Entry, &'static str>> {
        let s = query.trim();
        let upper = s.to_ascii_uppercase();
        let hex = upper
            .strip_prefix("U+")
            .or_else(|| upper.strip_prefix("0X"))
            .or_else(|| {
                (upper.len() >= 4
                    && upper.bytes().all(|b| b.is_ascii_hexdigit())
                    && upper.bytes().any(|b| b.is_ascii_digit()))
                .then_some(upper.as_str())
            });
        let glyph = if let Some(hex) = hex {
            let Some(c) = u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) else {
                return Some(Err(
                    "Enter a Unicode scalar value from U+0000 to U+10FFFF, excluding surrogates.",
                ));
            };
            c
        } else {
            let mut chars = s.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return self.0[self.1..]
                    .binary_search_by(|e| e.glyph.as_str().cmp(s))
                    .ok()
                    .map(|i| Ok(self.0[self.1 + i].clone()));
            }
            c
        };
        if let Ok(i) = self.0[..self.1].binary_search_by(|e| e.glyph.cmp(&glyph.to_string())) {
            return Some(Ok(self.0[i].clone()));
        }
        let mut buf = [0u8; 128];
        let len = unsafe {
            mundane_extended_name(glyph as i32, buf.as_mut_ptr().cast(), buf.len() as i32)
        };
        let name = if len > 0 {
            String::from_utf8_lossy(&buf[..len as usize]).into_owned()
        } else {
            "Unnamed character".into()
        };
        Some(Ok(Entry {
            glyph: glyph.to_string(),
            name,
            group: unsafe { mundane_character_group(glyph as i32) } as usize,
            emoji: None,
        }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_scalars() {
        let c = Catalog::load().unwrap();
        let hits = c.find("musical G CLEF", 0);
        assert!(hits.iter().any(|&i| c.get(i).glyph == "𝄞"));
        for query in ["u+1d11e", "1D11E", "𝄞"] {
            let e = c.exact(query).unwrap().unwrap();
            assert_eq!(e.name, "MUSICAL SYMBOL G CLEF");
            assert_eq!(e.code(), "U+1D11E")
        }
        for invalid in ["U+D800", "U+110000", "U+QQ", "U+"] {
            assert!(c.exact(invalid).unwrap().is_err())
        }
        let woman = "👩‍💻";
        assert!(
            c.find("technologist", 8)
                .iter()
                .any(|&i| c.get(i).glyph == woman)
        );
        assert_eq!(
            c.exact(woman).unwrap().unwrap().code(),
            "U+1F469 U+200D U+1F4BB"
        );
        assert!(
            c.find("LATIN CAPITAL LETTER A", 1)
                .iter()
                .any(|&i| c.get(i).glyph == "A")
        );
        assert!(c.find("LATIN CAPITAL LETTER A", 2).is_empty());
        assert!(c.exact("face").is_none());
        assert!(c.exact("U+E000").unwrap().is_ok());
    }
}
