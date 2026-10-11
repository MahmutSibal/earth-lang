//! The user's settings and signed-in profile, kept in a small JSON file:
//! `%APPDATA%\Earth Studio\settings.json` on Windows,
//! `~/Library/Application Support/Earth Studio` on macOS and
//! `~/.config/earth-studio` elsewhere.

use std::fs;
use std::path::PathBuf;

use earth::Lang;
use serde_json::{json, Value};

use crate::auth::{Profile, Provider};

pub const MIN_FONT_SIZE: f32 = 11.0;
pub const MAX_FONT_SIZE: f32 = 24.0;

#[derive(Clone, Debug, PartialEq)]
pub struct Settings {
    /// The language of Studio's own interface.
    pub lang: Lang,
    pub font_size: f32,
    pub auto_update: bool,
    pub profile: Option<Profile>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            lang: Lang::Tr,
            font_size: 15.0,
            auto_update: true,
            profile: None,
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
    let home = || std::env::var_os("HOME").map(PathBuf::from);
    let folder = if cfg!(windows) {
        std::env::var_os("APPDATA").map(|folder| PathBuf::from(folder).join("Earth Studio"))
    } else if cfg!(target_os = "macos") {
        home().map(|home| home.join("Library/Application Support/Earth Studio"))
    } else {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .or_else(|| home().map(|home| home.join(".config")))
            .map(|folder| folder.join("earth-studio"))
    };
    Some(folder?.join("settings.json"))
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
