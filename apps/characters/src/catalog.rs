use std::ffi::{c_char, c_void};
#[derive(Clone)]
pub struct Entry {
    pub glyph: char,
    pub name: String,
}
impl Entry {
    pub fn code(&self) -> String {
        format!("U+{:04X}", self.glyph as u32)
    }
}
unsafe extern "C" {
    fn mundane_names(
        data: *mut c_void,
        add: extern "C" fn(*mut c_void, i32, *const c_char, i32),
    ) -> i32;
    fn mundane_extended_name(code: i32, name: *mut c_char, capacity: i32) -> i32;
}
extern "C" fn collect(context: *mut c_void, code: i32, name: *const c_char, len: i32) {
    if let Some(glyph) = char::from_u32(code as u32) {
        // ICU calls synchronously with a counted invariant-ASCII name.
        let bytes = unsafe { std::slice::from_raw_parts(name.cast(), len as usize) };
        let entries = unsafe { &mut *context.cast::<Vec<Entry>>() };
        entries.push(Entry {
            glyph,
            name: String::from_utf8_lossy(bytes).into_owned(),
        })
    }
}
pub struct Catalog(Vec<Entry>);
impl Catalog {
    pub fn load() -> Result<Self, String> {
        let mut entries = Vec::<Entry>::new();
        if unsafe { mundane_names((&mut entries as *mut Vec<Entry>).cast(), collect) } == 0 {
            return Err("Could not read character names from ICU.".into());
        }
        Ok(Self(entries))
    }
    pub fn find(&self, query: &str) -> Vec<usize> {
        let words: Vec<_> = query
            .split_ascii_whitespace()
            .map(str::to_ascii_uppercase)
            .collect();
        self.0
            .iter()
            .enumerate()
            .filter(|(_, e)| words.iter().all(|w| e.name.contains(w)))
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
                return None;
            }
            c
        };
        if let Ok(i) = self.0.binary_search_by_key(&glyph, |e| e.glyph) {
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
        Some(Ok(Entry { glyph, name }))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn names_and_scalars() {
        let c = Catalog::load().unwrap();
        let hits = c.find("musical G CLEF");
        assert!(hits.iter().any(|&i| c.get(i).glyph == '𝄞'));
        for query in ["u+1d11e", "1D11E", "𝄞"] {
            let e = c.exact(query).unwrap().unwrap();
            assert_eq!(e.name, "MUSICAL SYMBOL G CLEF");
            assert_eq!(e.code(), "U+1D11E")
        }
        for invalid in ["U+D800", "U+110000", "U+QQ", "U+"] {
            assert!(c.exact(invalid).unwrap().is_err())
        }
        assert!(c.exact("face").is_none());
        assert!(c.exact("U+E000").unwrap().is_ok());
    }
}
