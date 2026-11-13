//! Keys written as text in the model, such as `command+s`, read as a [`Hotkey`].

use ravenrs_core::Key;
use ravenrs_widgets::Hotkey;

/// Reads keys such as `command+shift+s`, `alt+c` or `f9`.
pub(crate) fn hotkey_of(text: &str) -> Option<Hotkey> {
    let mut command = false;
    let mut shift = false;
    let mut alt = false;
    let mut key = None;
    for part in text.split('+') {
        match part.trim().to_lowercase().as_str() {
            "command" | "cmd" | "ctrl" => command = true,
            "shift" => shift = true,
            "alt" | "option" => alt = true,
            "delete" => key = Some(Key::Delete),
            "enter" => key = Some(Key::Enter),
            "esc" | "escape" => key = Some(Key::Escape),
            other => {
                let function = other.strip_prefix('f').and_then(|n| n.parse::<u8>().ok());
                key = Some(match function {
                    Some(number) if (1..=24).contains(&number) => Key::Function(number),
                    _ => Key::Character(other.to_owned()),
                });
            }
        }
    }
    let key = key?;
    let hotkey = match (&key, command, alt) {
        (Key::Character(letter), true, _) => Hotkey::command(letter),
        (Key::Character(letter), false, true) => Hotkey::alt(letter),
        _ => Hotkey::key(key),
    };
    Some(if shift { hotkey.shift() } else { hotkey })
}

#[cfg(test)]
mod tests {
    use ravenrs_core::Key;
    use ravenrs_widgets::Hotkey;

    use super::hotkey_of;

    #[test]
    fn keys_are_read_from_their_words() {
        let saved = hotkey_of("command+shift+s");
        assert_eq!(saved, Some(Hotkey::command("s").shift()));
        assert_eq!(hotkey_of("Delete"), Some(Hotkey::key(Key::Delete)));
        assert_eq!(hotkey_of("command+s"), Some(Hotkey::command("s")));
    }
}
