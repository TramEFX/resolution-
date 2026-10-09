//! Keyboard maps are data, not code: a TOML table of `keys → Resolution command id`.
//! The DaVinci Resolve map ships built in; users can load their own on top.

use std::collections::HashMap;

use serde::Deserialize;

/// The built-in DaVinci Resolve map.
pub const RESOLVE_TOML: &str = include_str!("../../../keymaps/resolve.toml");

#[derive(Debug, thiserror::Error)]
pub enum KeymapError {
    #[error("keymap is not valid TOML: {0}")]
    Parse(#[from] toml::de::Error),
    #[error("`{keys}` is bound to unknown command `{command}`")]
    UnknownCommand { keys: String, command: String },
    #[error("`{0}` is bound more than once")]
    Duplicate(String),
}

#[derive(Debug, Deserialize)]
pub struct Meta {
    pub name: String,
    #[serde(default)]
    pub resolve_version: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Binding {
    pub keys: String,
    pub command: String,
    /// Checked against a real Resolve install.
    #[serde(default)]
    pub verified: bool,
}

#[derive(Debug, Deserialize)]
struct File {
    meta: Meta,
    #[serde(default, rename = "bind")]
    binds: Vec<Binding>,
}

#[derive(Debug)]
pub struct Keymap {
    pub meta: Meta,
    pub bindings: Vec<Binding>,
    by_keys: HashMap<String, usize>,
}

impl Keymap {
    /// Parse and validate: every command must be in Resolution's vocabulary, every key unique.
    pub fn parse(src: &str) -> Result<Self, KeymapError> {
        let file: File = toml::from_str(src)?;
        let mut by_keys = HashMap::new();
        for (i, b) in file.binds.iter().enumerate() {
            if resolution_engine_api::commands::find(&b.command).is_none() {
                return Err(KeymapError::UnknownCommand { keys: b.keys.clone(), command: b.command.clone() });
            }
            if by_keys.insert(normalize(&b.keys), i).is_some() {
                return Err(KeymapError::Duplicate(b.keys.clone()));
            }
        }
        Ok(Self { meta: file.meta, bindings: file.binds, by_keys })
    }

    pub fn resolve() -> Self {
        Self::parse(RESOLVE_TOML).unwrap_or_else(|e| panic!("built-in Resolve keymap is invalid: {e}"))
    }

    /// The command bound to `keys` (`"Mod+B"`, `"shift+f11"`…).
    pub fn command_for(&self, keys: &str) -> Option<&str> {
        self.by_keys.get(&normalize(keys)).map(|&i| self.bindings[i].command.as_str())
    }

    /// Every key bound to `command`.
    pub fn keys_for<'a>(&'a self, command: &'a str) -> impl Iterator<Item = &'a str> + 'a {
        self.bindings.iter().filter(move |b| b.command == command).map(|b| b.keys.as_str())
    }
}

/// Canonical form: modifiers in a fixed order, case-insensitive.
fn normalize(keys: &str) -> String {
    const ORDER: [&str; 4] = ["mod", "ctrl", "alt", "shift"];
    let parts: Vec<String> = keys.split('+').map(|p| p.trim().to_ascii_lowercase()).collect();
    let (mut mods, rest): (Vec<_>, Vec<_>) = parts.into_iter().partition(|p| ORDER.contains(&p.as_str()));
    mods.sort_by_key(|m| ORDER.iter().position(|o| o == m));
    mods.into_iter().chain(rest).collect::<Vec<_>>().join("+")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builtin_resolve_map_is_valid() {
        let k = Keymap::resolve();
        assert_eq!(k.command_for("Mod+B"), Some("timeline.split"));
        assert_eq!(k.command_for("shift+f11"), Some("edit.fitToFill"));
        assert_eq!(k.command_for("Shift+Mod+Z"), Some("edit.redo"));
        assert!(k.keys_for("timeline.rippleDelete").count() >= 2);
    }

    #[test]
    fn rejects_unknown_commands_and_duplicates() {
        let bad = "[meta]\nname='x'\n[[bind]]\nkeys='A'\ncommand='no.such'\n";
        assert!(matches!(Keymap::parse(bad), Err(KeymapError::UnknownCommand { .. })));
        let dup = "[meta]\nname='x'\n[[bind]]\nkeys='Mod+A'\ncommand='edit.undo'\n[[bind]]\nkeys='mod+a'\ncommand='edit.redo'\n";
        assert!(matches!(Keymap::parse(dup), Err(KeymapError::Duplicate(_))));
    }
}
