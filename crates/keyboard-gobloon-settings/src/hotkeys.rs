//! The Hotkeys tab: every action with its hotkey, a record button and a remove button.
//!
//! Recording reads the keys held down from the system (egui knows no Pause key and no left and
//! right modifiers), collects every key pressed until all are released, and makes a hotkey of
//! them. Meanwhile the running app's hotkeys rest, so their keys reach the recorder.

use eframe::egui::{self, RichText};
use gob_i18n::I18n;
use gob_ui::icon;
use kbg_settings::{Action, Hotkey, Hotkeys, key_name};

/// VK_ESCAPE: pressed alone while recording, it cancels.
const ESCAPE: u16 = 0x1B;

#[derive(Default)]
pub struct Recorder {
    /// The action being recorded and the keys pressed so far, in order.
    recording: Option<(Action, Vec<u16>)>,
}

fn action_key(action: Action) -> &'static str {
    match action {
        Action::SwitchLayout => "action-switch-layout",
        Action::ConvertLast => "action-convert-last",
        Action::ConvertSelection => "action-convert-selection",
        Action::ChangeCase => "action-change-case",
        Action::ToggleEnabled => "action-toggle-enabled",
    }
}

fn chip(ui: &mut egui::Ui, text: impl Into<egui::WidgetText>) {
    egui::Frame::new()
        .fill(ui.visuals().faint_bg_color)
        .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
        .corner_radius(4)
        .inner_margin(egui::Margin::symmetric(6, 2))
        .show(ui, |ui| ui.label(text));
}

impl Recorder {
    fn start(&mut self, action: Action) {
        self.recording = Some((action, Vec::new()));
        kbg_platform::notify_recording(true);
    }

    /// Ends recording without a change (also when the window closes).
    pub fn stop(&mut self) {
        if self.recording.take().is_some() {
            kbg_platform::notify_recording(false);
        }
    }

    /// Draws the tab. `Some` when the hotkeys changed and must be saved, with a note for the
    /// status line when a hotkey moved from another action.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        hotkeys: &mut Hotkeys,
    ) -> Option<Option<String>> {
        let mut result = None;
        ui.label(RichText::new(i18n.tr("hotkeys-intro")).weak());
        ui.add_space(gob_ui::SPACE_M);
        egui::Grid::new("hotkeys")
            .num_columns(2)
            .spacing([gob_ui::SPACE_L, gob_ui::SPACE_S])
            .show(ui, |ui| {
                for action in Action::ALL {
                    ui.label(i18n.tr(action_key(action)));
                    ui.horizontal(|ui| {
                        if let Some(change) = self.row(ui, i18n, hotkeys, action) {
                            result = Some(change);
                        }
                    });
                    ui.end_row();
                }
            });
        ui.add_space(gob_ui::SPACE_L);
        if ui
            .button(format!(
                "{}  {}",
                icon::ARROW_COUNTER_CLOCKWISE,
                i18n.tr("keys-reset")
            ))
            .clicked()
        {
            self.stop();
            *hotkeys = Hotkeys::default();
            result = Some(None);
        }
        result
    }

    fn row(
        &mut self,
        ui: &mut egui::Ui,
        i18n: &I18n,
        hotkeys: &mut Hotkeys,
        action: Action,
    ) -> Option<Option<String>> {
        let recording = match &mut self.recording {
            Some((a, keys)) if *a == action => Some(keys),
            _ => None,
        };
        let Some(keys) = recording else {
            match hotkeys.get(action) {
                Some(hotkey) => chip(ui, hotkey.to_string()),
                None if action == Action::SwitchLayout => {
                    let keys = kbg_platform::system_switch_keys().join(", ");
                    ui.label(
                        RichText::new(i18n.trf("switch-system", &[("keys", keys.into())])).weak(),
                    );
                }
                None => {
                    ui.label(RichText::new(i18n.tr("keys-none")).weak());
                }
            }
            if gob_ui::record_button(ui, &i18n.tr("keys-record")).clicked() {
                self.stop();
                self.start(action);
            }
            if hotkeys.get(action).is_some()
                && gob_ui::icon_button(ui, true, icon::X, &i18n.tr("keys-clear")).clicked()
            {
                hotkeys.set(action, None);
                return Some(None);
            }
            return None;
        };

        // Recording: show the keys pressed so far, finish when all are released.
        let held = kbg_platform::held_keys();
        for k in &held {
            if !keys.contains(k) {
                keys.push(*k);
            }
        }
        if keys.is_empty() {
            ui.label(RichText::new(i18n.tr("keys-press")).italics());
        } else {
            let text: Vec<String> = keys.iter().map(|&k| key_name(k)).collect();
            chip(ui, text.join(" + "));
        }
        let cancel = gob_ui::icon_button(ui, true, icon::X, &i18n.tr("keys-cancel")).clicked();
        ui.ctx().request_repaint();
        if cancel || (held.is_empty() && keys.as_slice() == [ESCAPE]) {
            self.stop();
            return None;
        }
        if !held.is_empty() || keys.is_empty() {
            return None;
        }
        // All keys released: a hotkey, or (two plain keys) try again.
        let Some(hotkey) = Hotkey::from_keys(keys) else {
            keys.clear();
            return None;
        };
        self.stop();
        let note = hotkeys.bound_elsewhere(action, hotkey).map(|other| {
            hotkeys.set(other, None);
            i18n.trf(
                "keys-moved",
                &[
                    ("keys", hotkey.to_string().into()),
                    ("action", i18n.tr(action_key(other)).into()),
                ],
            )
        });
        hotkeys.set(action, Some(hotkey));
        Some(note)
    }
}
