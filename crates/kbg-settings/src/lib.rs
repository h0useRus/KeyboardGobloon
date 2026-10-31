//! Keyboard Gobloon's settings: TOML text in `settings.gob`, read by the app (again whenever the
//! settings window saves) and written by the settings window. Unknown or broken values fall
//! back to their defaults, so an old or hand-edited file never stops the app.

mod hotkey;

use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

pub use hotkey::{Hotkey, is_modifier, key_code, key_name};

pub const FILE: &str = "settings.gob";

/// Every action a hotkey can start. All of them can be bound in the settings window.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Action {
    /// Switch to the next layout. Unbound: the Windows hotkey switches, the app only follows.
    SwitchLayout,
    /// Retype the last word in the next layout.
    ConvertLast,
    /// Retype the selected text in the next layout.
    ConvertSelection,
    /// Cycle the selected text through lower, UPPER and Title case.
    ChangeCase,
    /// Pause or resume Keyboard Gobloon (the hotkey itself keeps working).
    ToggleEnabled,
}

impl Action {
    pub const ALL: [Action; 5] = [
        Action::SwitchLayout,
        Action::ConvertLast,
        Action::ConvertSelection,
        Action::ChangeCase,
        Action::ToggleEnabled,
    ];
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Hotkeys {
    #[serde(with = "hotkey::text")]
    pub switch_layout: Option<Hotkey>,
    #[serde(with = "hotkey::text")]
    pub convert_last: Option<Hotkey>,
    #[serde(with = "hotkey::text")]
    pub convert_selection: Option<Hotkey>,
    #[serde(with = "hotkey::text")]
    pub change_case: Option<Hotkey>,
    #[serde(with = "hotkey::text")]
    pub toggle_enabled: Option<Hotkey>,
}

/// VK_PAUSE
const PAUSE: u16 = 0x13;

impl Default for Hotkeys {
    fn default() -> Self {
        Self {
            switch_layout: None,
            convert_last: Some(Hotkey::key(PAUSE)),
            convert_selection: Some(Hotkey::key(PAUSE).shift()),
            change_case: Some(Hotkey::key(PAUSE).alt()),
            toggle_enabled: None,
        }
    }
}

impl Hotkeys {
    pub fn get(&self, action: Action) -> Option<Hotkey> {
        match action {
            Action::SwitchLayout => self.switch_layout,
            Action::ConvertLast => self.convert_last,
            Action::ConvertSelection => self.convert_selection,
            Action::ChangeCase => self.change_case,
            Action::ToggleEnabled => self.toggle_enabled,
        }
    }

    pub fn set(&mut self, action: Action, hotkey: Option<Hotkey>) {
        *match action {
            Action::SwitchLayout => &mut self.switch_layout,
            Action::ConvertLast => &mut self.convert_last,
            Action::ConvertSelection => &mut self.convert_selection,
            Action::ChangeCase => &mut self.change_case,
            Action::ToggleEnabled => &mut self.toggle_enabled,
        } = hotkey;
    }

    /// The other action already bound to `hotkey`, if any.
    pub fn bound_elsewhere(&self, action: Action, hotkey: Hotkey) -> Option<Action> {
        Action::ALL
            .into_iter()
            .find(|&a| a != action && self.get(a) == Some(hotkey))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// UI language code; empty follows the system.
    pub language: String,
    pub theme: Theme,
    /// Retype words typed in the wrong layout as they end (needs the languages' packs).
    pub auto_switch: bool,
    pub hotkeys: Hotkeys,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            language: String::new(),
            theme: Theme::default(),
            auto_switch: true,
            hotkeys: Hotkeys::default(),
        }
    }
}

/// Folder of settings and the log: the exe's folder when it holds a `settings.gob` (portable
/// copy), else `default` (the user's application data, from the platform).
pub fn data_dir(default: impl FnOnce() -> Option<PathBuf>) -> Option<&'static Path> {
    static DIR: OnceLock<Option<PathBuf>> = OnceLock::new();
    DIR.get_or_init(|| {
        let local = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(Path::to_path_buf))
            .filter(|dir| dir.join(FILE).is_file());
        local.or_else(default)
    })
    .as_deref()
}

impl Settings {
    /// Reads `dir/settings.gob`; missing or unreadable values are defaults.
    pub fn load(dir: &Path) -> Self {
        std::fs::read_to_string(dir.join(FILE))
            .ok()
            .map(|text| Self::parse(&text))
            .unwrap_or_default()
    }

    /// Like [`Settings::load`] from text: a broken table or value is replaced by its default
    /// instead of losing the whole file.
    pub fn parse(text: &str) -> Self {
        if let Ok(settings) = toml::from_str(text) {
            return settings;
        }
        let Ok(table) = toml::from_str::<toml::Table>(text) else {
            return Self::default();
        };
        let mut settings = Self::default();
        let field = |name: &str| table.get(name).cloned();
        if let Some(Ok(v)) = field("language").map(toml::Value::try_into) {
            settings.language = v;
        }
        if let Some(Ok(v)) = field("theme").map(toml::Value::try_into) {
            settings.theme = v;
        }
        if let Some(Ok(v)) = field("auto_switch").map(toml::Value::try_into) {
            settings.auto_switch = v;
        }
        if let Some(toml::Value::Table(hotkeys)) = field("hotkeys") {
            for (name, value) in hotkeys {
                let one = toml::Table::from_iter([(name, value)]);
                if let Ok(parsed) = toml::Value::Table(one).try_into::<Hotkeys>() {
                    for action in Action::ALL {
                        let default = Hotkeys::default().get(action);
                        if parsed.get(action) != default {
                            settings.hotkeys.set(action, parsed.get(action));
                        }
                    }
                }
            }
        }
        settings
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        std::fs::create_dir_all(dir)?;
        let text = toml::to_string_pretty(self).map_err(std::io::Error::other)?;
        // Written whole and then renamed, so the app never reads half a file.
        let tmp = dir.join(format!("{FILE}.tmp"));
        std::fs::write(&tmp, text)?;
        std::fs::rename(tmp, dir.join(FILE))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut s = Settings {
            language: "ru".into(),
            theme: Theme::Dark,
            auto_switch: false,
            ..Default::default()
        };
        s.hotkeys.switch_layout = Some("RCtrl".parse().unwrap());
        s.hotkeys.change_case = None;
        let text = toml::to_string_pretty(&s).unwrap();
        assert!(text.contains("switch_layout = \"RCtrl\""), "{text}");
        assert!(text.contains("change_case = \"\""), "{text}");
        assert_eq!(Settings::parse(&text), s);
    }

    #[test]
    fn defaults_for_missing_and_broken_values() {
        assert_eq!(Settings::parse(""), Settings::default());
        let s = Settings::parse(
            "theme = \"purple\"\n[hotkeys]\nconvert_last = \"F9\"\nchange_case = \"Hyper+Q\"\n",
        );
        assert_eq!(s.theme, Theme::System);
        assert!(s.auto_switch);
        assert!(
            !Settings::parse(
                "theme = 1
auto_switch = false
"
            )
            .auto_switch
        );
        assert_eq!(s.hotkeys.convert_last, Some("F9".parse().unwrap()));
        assert_eq!(s.hotkeys.change_case, Hotkeys::default().change_case);
    }

    #[test]
    fn conflicts() {
        let h = Hotkeys::default();
        assert_eq!(
            h.bound_elsewhere(Action::SwitchLayout, Hotkey::key(PAUSE)),
            Some(Action::ConvertLast)
        );
        assert_eq!(
            h.bound_elsewhere(Action::ConvertLast, Hotkey::key(PAUSE)),
            None
        );
    }
}
