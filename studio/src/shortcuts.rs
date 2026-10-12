//! Keyboard shortcuts: every action has a default, and the user can bind
//! another in the settings. Bindings are stored as text such as
//! `Ctrl+Shift+F5`, which is also how they are shown.

use std::collections::BTreeMap;

use eframe::egui::{Key, KeyboardShortcut, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Action {
    Run,
    Debug,
    Stop,
    Continue,
    StepOver,
    StepInto,
    StepOut,
    ToggleBreakpoint,
    Save,
    NewFile,
    Assistant,
    Complete,
}

impl Action {
    pub const ALL: [Action; 12] = [
        Action::Run,
        Action::Debug,
        Action::Stop,
        Action::Continue,
        Action::StepOver,
        Action::StepInto,
        Action::StepOut,
        Action::ToggleBreakpoint,
        Action::Save,
        Action::NewFile,
        Action::Assistant,
        Action::Complete,
    ];

    /// The name stored in the settings file.
    pub fn id(self) -> &'static str {
        match self {
            Action::Run => "run",
            Action::Debug => "debug",
            Action::Stop => "stop",
            Action::Continue => "continue",
            Action::StepOver => "step_over",
            Action::StepInto => "step_into",
            Action::StepOut => "step_out",
            Action::ToggleBreakpoint => "toggle_breakpoint",
            Action::Save => "save",
            Action::NewFile => "new_file",
            Action::Assistant => "assistant",
            Action::Complete => "complete",
        }
    }

    pub fn default_shortcut(self) -> KeyboardShortcut {
        let key = |modifiers, key| KeyboardShortcut::new(modifiers, key);
        match self {
            Action::Run => key(Modifiers::NONE, Key::F5),
            Action::Debug => key(Modifiers::NONE, Key::F6),
            Action::Stop => key(Modifiers::SHIFT, Key::F5),
            Action::Continue => key(Modifiers::NONE, Key::F8),
            Action::StepOver => key(Modifiers::NONE, Key::F10),
            Action::StepInto => key(Modifiers::NONE, Key::F11),
            Action::StepOut => key(Modifiers::SHIFT, Key::F11),
            Action::ToggleBreakpoint => key(Modifiers::NONE, Key::F9),
            Action::Save => key(Modifiers::COMMAND, Key::S),
            Action::NewFile => key(Modifiers::COMMAND, Key::N),
            Action::Assistant => key(Modifiers::COMMAND, Key::I),
            Action::Complete => key(Modifiers::COMMAND, Key::Space),
        }
    }
}

/// The shortcut of every action, defaults filled in.
#[derive(Clone, Debug, PartialEq)]
pub struct Bindings {
    shortcuts: BTreeMap<Action, KeyboardShortcut>,
}

impl Default for Bindings {
    fn default() -> Self {
        Self {
            shortcuts: Action::ALL
                .into_iter()
                .map(|action| (action, action.default_shortcut()))
                .collect(),
        }
    }
}

impl Bindings {
    /// Bindings from the settings: action id → shortcut text. Unknown or
    /// unreadable entries keep the default.
    pub fn from_settings(saved: &BTreeMap<String, String>) -> Self {
        let mut bindings = Self::default();
        for action in Action::ALL {
            if let Some(shortcut) = saved.get(action.id()).and_then(|text| parse(text)) {
                bindings.shortcuts.insert(action, shortcut);
            }
        }
        bindings
    }

    /// Only the bindings that differ from the defaults, for saving.
    pub fn to_settings(&self) -> BTreeMap<String, String> {
        self.shortcuts
            .iter()
            .filter(|(action, shortcut)| action.default_shortcut() != **shortcut)
            .map(|(action, shortcut)| (action.id().to_owned(), format(shortcut)))
            .collect()
    }

    pub fn get(&self, action: Action) -> KeyboardShortcut {
        self.shortcuts
            .get(&action)
            .copied()
            .unwrap_or_else(|| action.default_shortcut())
    }

    pub fn set(&mut self, action: Action, shortcut: KeyboardShortcut) {
        self.shortcuts.insert(action, shortcut);
    }

    /// Another action already using `shortcut`, if any.
    pub fn conflict(&self, action: Action, shortcut: KeyboardShortcut) -> Option<Action> {
        self.shortcuts
            .iter()
            .find(|(other, bound)| **other != action && **bound == shortcut)
            .map(|(other, _)| *other)
    }
}

/// `Ctrl+Shift+F5` style text for a shortcut. Ctrl means Command on a Mac.
pub fn format(shortcut: &KeyboardShortcut) -> String {
    let mut parts = Vec::new();
    let modifiers = shortcut.modifiers;
    if modifiers.command || modifiers.ctrl || modifiers.mac_cmd {
        parts.push("Ctrl");
    }
    if modifiers.alt {
        parts.push("Alt");
    }
    if modifiers.shift {
        parts.push("Shift");
    }
    parts.push(shortcut.logical_key.name());
    parts.join("+")
}

pub fn parse(text: &str) -> Option<KeyboardShortcut> {
    let mut modifiers = Modifiers::NONE;
    let mut key = None;
    for part in text.split('+').map(str::trim) {
        match part.to_lowercase().as_str() {
            "ctrl" | "cmd" | "command" => modifiers |= Modifiers::COMMAND,
            "alt" => modifiers |= Modifiers::ALT,
            "shift" => modifiers |= Modifiers::SHIFT,
            _ => key = Some(Key::from_name(part)?),
        }
    }
    Some(KeyboardShortcut::new(modifiers, key?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_survive_text() {
        for action in Action::ALL {
            let shortcut = action.default_shortcut();
            assert_eq!(parse(&format(&shortcut)), Some(shortcut), "{action:?}");
        }
        assert_eq!(format(&parse("ctrl+shift+F5").unwrap()), "Ctrl+Shift+F5");
        assert_eq!(parse("Ctrl+Nonsense"), None);
    }

    #[test]
    fn only_changes_are_saved() {
        let mut bindings = Bindings::default();
        assert!(bindings.to_settings().is_empty());
        bindings.set(Action::Run, parse("Ctrl+R").unwrap());
        let saved = bindings.to_settings();
        assert_eq!(saved.get("run").map(String::as_str), Some("Ctrl+R"));
        assert_eq!(Bindings::from_settings(&saved), bindings);
        assert_eq!(
            bindings.conflict(Action::Save, parse("Ctrl+R").unwrap()),
            Some(Action::Run)
        );
    }
}
