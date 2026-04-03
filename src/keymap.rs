// USB HID keyboard scancodes (US layout)
// Reference: USB HID Usage Tables, Section 10 (Keyboard/Keypad Page 0x07)

pub const KEY_ENTER: u8 = 0x28;

const MOD_NONE: u8 = 0x00;
const MOD_LCTRL: u8 = 0x01;
const MOD_LSHIFT: u8 = 0x02;

/// Returns (modifier, keycode) for a character, or None if unmapped.
pub fn lookup(ch: char) -> Option<(u8, u8)> {
    let (modifier, keycode) = match ch {
        '\n' => (MOD_NONE, KEY_ENTER),
        '\t' => (MOD_NONE, 0x2b),
        '\x08' | '\x7f' => (MOD_NONE, 0x2a), // backspace / delete
        // Ctrl+A through Ctrl+Z (0x01-0x1a), excluding \t \n \x08 matched above
        '\x01'..='\x1a' => (MOD_LCTRL, 0x04 + (ch as u8 - 0x01)),
        'a'..='z' => (MOD_NONE, 0x04 + (ch as u8 - b'a')),
        'A'..='Z' => (MOD_LSHIFT, 0x04 + (ch as u8 - b'A')),
        '1'..='9' => (MOD_NONE, 0x1e + (ch as u8 - b'1')),
        '0' => (MOD_NONE, 0x27),
        ' ' => (MOD_NONE, 0x2c),

        // Punctuation (unshifted keys)
        '-' => (MOD_NONE, 0x2d),
        '=' => (MOD_NONE, 0x2e),
        '[' => (MOD_NONE, 0x2f),
        ']' => (MOD_NONE, 0x30),
        '\\' => (MOD_NONE, 0x31),
        ';' => (MOD_NONE, 0x33),
        '\'' => (MOD_NONE, 0x34),
        '`' => (MOD_NONE, 0x35),
        ',' => (MOD_NONE, 0x36),
        '.' => (MOD_NONE, 0x37),
        '/' => (MOD_NONE, 0x38),

        // Shifted punctuation
        '!' => (MOD_LSHIFT, 0x1e),
        '@' => (MOD_LSHIFT, 0x1f),
        '#' => (MOD_LSHIFT, 0x20),
        '$' => (MOD_LSHIFT, 0x21),
        '%' => (MOD_LSHIFT, 0x22),
        '^' => (MOD_LSHIFT, 0x23),
        '&' => (MOD_LSHIFT, 0x24),
        '*' => (MOD_LSHIFT, 0x25),
        '(' => (MOD_LSHIFT, 0x26),
        ')' => (MOD_LSHIFT, 0x27),
        '_' => (MOD_LSHIFT, 0x2d),
        '+' => (MOD_LSHIFT, 0x2e),
        '{' => (MOD_LSHIFT, 0x2f),
        '}' => (MOD_LSHIFT, 0x30),
        '|' => (MOD_LSHIFT, 0x31),
        ':' => (MOD_LSHIFT, 0x33),
        '"' => (MOD_LSHIFT, 0x34),
        '~' => (MOD_LSHIFT, 0x35),
        '<' => (MOD_LSHIFT, 0x36),
        '>' => (MOD_LSHIFT, 0x37),
        '?' => (MOD_LSHIFT, 0x38),

        _ => return None,
    };
    Some((modifier, keycode))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lowercase_letters() {
        assert_eq!(lookup('a'), Some((MOD_NONE, 0x04)));
        assert_eq!(lookup('z'), Some((MOD_NONE, 0x1d)));
    }

    #[test]
    fn uppercase_letters() {
        assert_eq!(lookup('A'), Some((MOD_LSHIFT, 0x04)));
        assert_eq!(lookup('Z'), Some((MOD_LSHIFT, 0x1d)));
    }

    #[test]
    fn digits() {
        assert_eq!(lookup('1'), Some((MOD_NONE, 0x1e)));
        assert_eq!(lookup('0'), Some((MOD_NONE, 0x27)));
    }

    #[test]
    fn shifted_punctuation() {
        assert_eq!(lookup('!'), Some((MOD_LSHIFT, 0x1e)));
        assert_eq!(lookup('?'), Some((MOD_LSHIFT, 0x38)));
    }

    #[test]
    fn ctrl_keys() {
        assert_eq!(lookup('\x01'), Some((MOD_LCTRL, 0x04))); // Ctrl+A
        assert_eq!(lookup('\x03'), Some((MOD_LCTRL, 0x06))); // Ctrl+C
        assert_eq!(lookup('\x1a'), Some((MOD_LCTRL, 0x1d))); // Ctrl+Z
    }

    #[test]
    fn unmapped_returns_none() {
        assert_eq!(lookup('å'), None);
        assert_eq!(lookup('€'), None);
    }
}
