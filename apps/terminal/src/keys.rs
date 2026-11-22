use gpui::Keystroke;

/// Encode non-text keys here. GPUI's text input handler delivers composed text.
pub fn encode(key: &Keystroke, application_cursor: bool) -> Option<Vec<u8>> {
    let mods = key.modifiers;
    if mods.platform {
        return None;
    }
    let name = key.key.as_str();
    let modifier = 1 + u8::from(mods.shift) + 2 * u8::from(mods.alt) + 4 * u8::from(mods.control);
    let final_byte = match name {
        "up" => Some('A'),
        "down" => Some('B'),
        "right" => Some('C'),
        "left" => Some('D'),
        "home" => Some('H'),
        "end" => Some('F'),
        "f1" => Some('P'),
        "f2" => Some('Q'),
        "f3" => Some('R'),
        "f4" => Some('S'),
        _ => None,
    };
    if let Some(end) = final_byte {
        let prefix = if modifier != 1 {
            format!("\x1b[1;{modifier}")
        } else if application_cursor || name.starts_with('f') {
            "\x1bO".into()
        } else {
            "\x1b[".into()
        };
        return Some(format!("{prefix}{end}").into_bytes());
    }
    let tilde = match name {
        "insert" => Some(2),
        "delete" => Some(3),
        "pageup" => Some(5),
        "pagedown" => Some(6),
        "f5" => Some(15),
        "f6" => Some(17),
        "f7" => Some(18),
        "f8" => Some(19),
        "f9" => Some(20),
        "f10" => Some(21),
        "f11" => Some(23),
        "f12" => Some(24),
        _ => None,
    };
    if let Some(code) = tilde {
        return Some(
            if modifier == 1 {
                format!("\x1b[{code}~")
            } else {
                format!("\x1b[{code};{modifier}~")
            }
            .into_bytes(),
        );
    }
    let mut bytes = match name {
        "enter" => vec![13],
        "backspace" => vec![127],
        "escape" => vec![27],
        "tab" if mods.shift => b"\x1b[Z".to_vec(),
        "tab" => vec![9],
        "space" if mods.control => vec![0],
        _ if mods.control && name.len() == 1 => {
            let c = name.as_bytes()[0].to_ascii_uppercase();
            if (b'@'..=b'_').contains(&c) {
                vec![c & 31]
            } else {
                return None;
            }
        }
        _ if mods.alt => key
            .key_char
            .as_deref()
            .or_else(|| (name.chars().count() == 1).then_some(name))?
            .as_bytes()
            .to_vec(),
        _ => return None,
    };
    if mods.alt {
        bytes.insert(0, 27);
    }
    Some(bytes)
}
