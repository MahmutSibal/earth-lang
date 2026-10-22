//! The user's settings and signed-in profile, kept in a small JSON file:
//! `%APPDATA%\Earth Studio\settings.json` on Windows,
//! `~/Library/Application Support/Earth Studio` on macOS and
//! `~/.config/earth-studio` elsewhere.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use earth::Lang;
use serde_json::{json, Value};

use crate::auth::{Profile, Provider};

pub const MIN_FONT_SIZE: f32 = 11.0;
pub const MAX_FONT_SIZE: f32 = 24.0;
pub const MIN_UI_SCALE: f32 = 0.8;
pub const MAX_UI_SCALE: f32 = 1.6;

/// Which side of the window a panel is on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Left,
    Right,
}

impl Side {
    fn id(self) -> &'static str {
        match self {
            Side::Left => "left",
            Side::Right => "right",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        match id {
            "left" => Some(Side::Left),
            "right" => Some(Side::Right),
            _ => None,
        }
    }
}

/// How the assistant talks to the user.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssistantMode {
    /// Answers questions and writes code.
    Assistant,
    /// Teaches: explains, gives hints and exercises, checks the user's work.
    Teacher,
}

/// How much the teacher assumes the user knows.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Beginner,
    Intermediate,
    Advanced,
}

impl Level {
    pub const ALL: [Level; 3] = [Level::Beginner, Level::Intermediate, Level::Advanced];

    pub fn id(self) -> &'static str {
        match self {
            Level::Beginner => "beginner",
            Level::Intermediate => "intermediate",
            Level::Advanced => "advanced",
        }
    }

    fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|level| level.id() == id)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// The language of Studio's own interface.
    pub lang: Lang,
    pub font_size: f32,
    pub auto_update: bool,
    pub profile: Option<Profile>,
    /// The assistant's AI service (`assistant::Service` id), its address
    /// for a custom service, and the model; empty until set up.
    pub ai_service: String,
    pub ai_base_url: String,
    pub ai_model: String,
    pub assistant_open: bool,
    /// `theme::ThemeName` id.
    pub theme: String,
    /// Scales the whole interface.
    pub ui_scale: f32,
    pub explorer_side: Side,
    pub assistant_side: Side,
    /// Shortcuts the user changed: action id → shortcut text.
    pub shortcuts: BTreeMap<String, String>,
    pub assistant_mode: AssistantMode,
    pub teacher_level: Level,
    /// Lessons the user finished with the teacher, by number.
    pub lessons_done: Vec<usize>,
    /// Optional Python interpreter path; empty means auto-detect.
    pub python_path: String,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            lang: Lang::Tr,
            font_size: 15.0,
            auto_update: true,
            profile: None,
            ai_service: String::new(),
            ai_base_url: String::new(),
            ai_model: String::new(),
            assistant_open: false,
            theme: "midnight".to_owned(),
            ui_scale: 1.0,
            explorer_side: Side::Left,
            assistant_side: Side::Right,
            shortcuts: BTreeMap::new(),
            assistant_mode: AssistantMode::Assistant,
            teacher_level: Level::Beginner,
            lessons_done: Vec::new(),
            python_path: String::new(),
        }
    }
}

impl Settings {
    /// The saved settings, or the defaults if there are none yet.
    pub fn load() -> Self {
        file()
            .and_then(|path| fs::read_to_string(path).ok())
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
            .map(|value| Self::from_json(&value))
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let Some(path) = file() else {
            return;
        };
        if let Some(folder) = path.parent() {
            let _ = fs::create_dir_all(folder);
        }
        if let Ok(text) = serde_json::to_string_pretty(&self.to_json()) {
            let _ = fs::write(path, text);
        }
    }

    /// Missing or invalid entries fall back to their defaults.
    fn from_json(value: &Value) -> Self {
        let defaults = Self::default();
        let profile = &value["profile"];
        let provider = match profile["provider"].as_str() {
            Some("google") => Some(Provider::Google),
            Some("github") => Some(Provider::GitHub),
            _ => None,
        };
        Self {
            lang: value["lang"]
                .as_str()
                .and_then(Lang::from_code)
                .unwrap_or(defaults.lang),
            font_size: value["font_size"]
                .as_f64()
                .map(|size| (size as f32).clamp(MIN_FONT_SIZE, MAX_FONT_SIZE))
                .unwrap_or(defaults.font_size),
            auto_update: value["auto_update"]
                .as_bool()
                .unwrap_or(defaults.auto_update),
            ai_service: text(&value["ai_service"]),
            ai_base_url: text(&value["ai_base_url"]),
            ai_model: text(&value["ai_model"]),
            assistant_open: value["assistant_open"]
                .as_bool()
                .unwrap_or(defaults.assistant_open),
            theme: value["theme"]
                .as_str()
                .filter(|id| crate::theme::ThemeName::from_id(id).is_some())
                .map_or(defaults.theme, str::to_owned),
            ui_scale: value["ui_scale"]
                .as_f64()
                .map(|scale| (scale as f32).clamp(MIN_UI_SCALE, MAX_UI_SCALE))
                .unwrap_or(defaults.ui_scale),
            explorer_side: value["explorer_side"]
                .as_str()
                .and_then(Side::from_id)
                .unwrap_or(defaults.explorer_side),
            assistant_side: value["assistant_side"]
                .as_str()
                .and_then(Side::from_id)
                .unwrap_or(defaults.assistant_side),
            shortcuts: value["shortcuts"]
                .as_object()
                .map(|map| {
                    map.iter()
                        .filter_map(|(action, shortcut)| {
                            Some((action.clone(), shortcut.as_str()?.to_owned()))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            assistant_mode: match value["assistant_mode"].as_str() {
                Some("teacher") => AssistantMode::Teacher,
                _ => AssistantMode::Assistant,
            },
            teacher_level: value["teacher_level"]
                .as_str()
                .and_then(Level::from_id)
                .unwrap_or(defaults.teacher_level),
            lessons_done: value["lessons_done"]
                .as_array()
                .map(|done| {
                    done.iter()
                        .filter_map(|n| n.as_u64().map(|n| n as usize))
                        .collect()
                })
                .unwrap_or_default(),
            python_path: text(&value["python_path"]),
            profile: provider.map(|provider| Profile {
                provider,
                name: text(&profile["name"]),
                account: text(&profile["account"]),
                avatar_url: text(&profile["avatar_url"]),
            }),
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "lang": match self.lang {
                Lang::Tr => "tr",
                Lang::En => "en",
            },
            "font_size": self.font_size,
            "auto_update": self.auto_update,
            "ai_service": self.ai_service,
            "ai_base_url": self.ai_base_url,
            "ai_model": self.ai_model,
            "assistant_open": self.assistant_open,
            "theme": self.theme,
            "ui_scale": self.ui_scale,
            "explorer_side": self.explorer_side.id(),
            "assistant_side": self.assistant_side.id(),
            "shortcuts": self.shortcuts,
            "assistant_mode": match self.assistant_mode {
                AssistantMode::Assistant => "assistant",
                AssistantMode::Teacher => "teacher",
            },
            "teacher_level": self.teacher_level.id(),
            "lessons_done": self.lessons_done,
            "python_path": self.python_path,
            "profile": self.profile.as_ref().map(|profile| json!({
                "provider": match profile.provider {
                    Provider::Google => "google",
                    Provider::GitHub => "github",
                },
                "name": profile.name,
                "account": profile.account,
                "avatar_url": profile.avatar_url,
            })),
        })
    }
}

fn text(value: &Value) -> String {
    value.as_str().unwrap_or_default().to_owned()
}

fn file() -> Option<PathBuf> {
    Some(folder()?.join("settings.json"))
}

/// Studio's own folder for settings and the like.
pub fn folder() -> Option<PathBuf> {
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    if cfg!(windows) {
        std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("Earth Studio"))
    } else if cfg!(target_os = "macos") {
        home().map(|home| home.join("Library/Application Support/Earth Studio"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|home| home.join(".config")))
            .map(|folder| folder.join("earth-studio"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_json() {
        let settings = Settings {
            lang: Lang::En,
            font_size: 18.0,
            auto_update: false,
            ai_service: "gemini".to_owned(),
            ai_base_url: String::new(),
            ai_model: "gemini-flash".to_owned(),
            theme: "light".to_owned(),
            ui_scale: 1.25,
            explorer_side: Side::Right,
            assistant_side: Side::Left,
            shortcuts: [("run".to_owned(), "Ctrl+R".to_owned())].into(),
            assistant_mode: AssistantMode::Teacher,
            teacher_level: Level::Intermediate,
            lessons_done: vec![1, 3],
            python_path: String::new(),
            assistant_open: true,
            profile: Some(Profile {
                provider: Provider::GitHub,
                name: "Mahmut".to_owned(),
                account: "@MahmutSibal".to_owned(),
                avatar_url: "https://example.com/a.png".to_owned(),
            }),
        };
        assert_eq!(Settings::from_json(&settings.to_json()), settings);
    }

    #[test]
    fn falls_back_to_defaults() {
        let settings = Settings::from_json(&json!({ "font_size": 400, "lang": "xx" }));
        assert_eq!(settings.font_size, MAX_FONT_SIZE);
        assert_eq!(settings.lang, Lang::Tr);
        assert!(settings.auto_update);
        assert_eq!(settings.profile, None);
    }
}
