//! Key presses as keymap chords ("Mod+B", "Shift+F11", "Left"), the notation `keymaps/*.toml` uses.

use egui::{Key, Modifiers};

/// The chord for `key` with `modifiers`, or `None` for keys the keymap never binds.
pub fn chord(key: Key, modifiers: Modifiers) -> Option<String> {
    let name = name(key)?;
    let mut parts = Vec::with_capacity(4);
    // `command` is Cmd on macOS and Ctrl elsewhere: the keymap's "Mod".
    if modifiers.command {
        parts.push("Mod");
    }
    // A Ctrl that is not the command key (macOS Control).
    if modifiers.ctrl && !modifiers.command {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    parts.push(name);
    Some(parts.join("+"))
}

fn name(key: Key) -> Option<&'static str> {
    use Key::*;
    Some(match key {
        ArrowLeft => "Left",
        ArrowRight => "Right",
        ArrowUp => "Up",
        ArrowDown => "Down",
        Space => "Space",
        Backspace => "Backspace",
        Delete => "Delete",
        Home => "Home",
        End => "End",
        Slash => "/",
        Semicolon => ";",
        Quote => "'",
        Backslash => "\\",
        OpenBracket => "[",
        CloseBracket => "]",
        // Mod+= zooms in; on many layouts the same key reports Plus.
        Equals | Plus => "=",
        Minus => "-",
        Num0 => "0",
        Num1 => "1",
        Num2 => "2",
        Num3 => "3",
        Num4 => "4",
        Num5 => "5",
        Num6 => "6",
        Num7 => "7",
        Num8 => "8",
        Num9 => "9",
        F1 => "F1",
        F2 => "F2",
        F3 => "F3",
        F4 => "F4",
        F5 => "F5",
        F6 => "F6",
        F7 => "F7",
        F8 => "F8",
        F9 => "F9",
        F10 => "F10",
        F11 => "F11",
        F12 => "F12",
        other => {
            let n = other.name();
            return (n.len() == 1 && n.chars().all(|c| c.is_ascii_alphabetic())).then_some(n);
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use resolution_keymap::Keymap;

    #[test]
    fn chords_use_the_keymap_notation() {
        assert_eq!(chord(Key::B, Modifiers::COMMAND).as_deref(), Some("Mod+B"));
        assert_eq!(chord(Key::F11, Modifiers::SHIFT).as_deref(), Some("Shift+F11"));
        assert_eq!(chord(Key::ArrowLeft, Modifiers::NONE).as_deref(), Some("Left"));
        assert_eq!(chord(Key::Num2, Modifiers::SHIFT).as_deref(), Some("Shift+2"));
        assert_eq!(chord(Key::Tab, Modifiers::NONE), None);
    }

    /// Every chord the UI produces for the Resolve keymap's main keys finds its command.
    #[test]
    fn resolve_keys_reach_their_commands() {
        let k = Keymap::resolve();
        let cases = [
            (Key::B, Modifiers::COMMAND, "timeline.split"),
            (Key::Z, Modifiers::COMMAND, "edit.undo"),
            (Key::Z, Modifiers::COMMAND | Modifiers::SHIFT, "edit.redo"),
            (Key::Space, Modifiers::NONE, "playback.playStop"),
            (Key::L, Modifiers::NONE, "playback.forward"),
            (Key::I, Modifiers::NONE, "mark.in"),
            (Key::F9, Modifiers::NONE, "edit.insert"),
            (Key::Delete, Modifiers::NONE, "timeline.rippleDelete"),
            (Key::Num4, Modifiers::SHIFT, "page.edit"),
            (Key::Equals, Modifiers::COMMAND, "view.zoomIn"),
        ];
        for (key, m, want) in cases {
            let c = chord(key, m).expect("chord");
            assert_eq!(k.command_for(&c), Some(want), "{c}");
        }
    }
}
