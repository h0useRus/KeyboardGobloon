//! Keyboard Gobloon's settings window. A process of its own, started from the tray: the app in
//! the background never loads a UI library. Every change is saved to settings.gob at once and
//! the running app is told to read it again.

// Release builds run without a console window.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod hotkeys;

use std::path::{Path, PathBuf};

use eframe::egui::{self, Color32, RichText, Vec2};
use gob_i18n::I18n;
use gob_ui::{Palette, Palettes, icon};
use kbg_settings::{Settings, Theme};

/// The window title: the app finds an open settings window by it (keyboard-gobloon main.rs).
pub const APP_NAME: &str = "Keyboard Gobloon";
pub const VERSION: &str = env!("KEYBOARD_GOBLOON_VERSION");
const AUTHOR: &str = "Denis Ibragimov";
const APP_EXE: &str = "KeyboardGobloon.exe";
const WEBSITE: &str = "https://h0userus.github.io/keyboard-gobloon/";
const REPO: &str = "https://github.com/h0useRus/KeyboardGobloon";
const DONATE_URL: &str = "https://www.paypal.com/paypalme/h0userus";
const LOGO_PNG: &[u8] = include_bytes!("../../../assets/icon/KeyboardGobloon-256.png");

/// The same languages as the app.
pub const LANGUAGES: &[(&str, &str)] = &[
    ("en", include_str!("../../../lang/en.ftl")),
    ("ru", include_str!("../../../lang/ru.ftl")),
];

/// Goblin green, as Joystick Gobloon.
const PALETTES: Palettes = Palettes {
    dark: Palette {
        accent: Color32::from_rgb(0x8c, 0xc8, 0x2c),
        panel: Color32::from_rgb(0x1b, 0x1d, 0x22),
        window: Color32::from_rgb(0x23, 0x26, 0x2c),
        faint: Color32::from_rgb(0x2a, 0x2d, 0x34),
    },
    light: Palette {
        accent: Color32::from_rgb(0x6f, 0x9e, 0x1e),
        panel: Color32::from_rgb(0xf4, 0xf5, 0xf7),
        window: Color32::from_rgb(0xff, 0xff, 0xff),
        faint: Color32::from_rgb(0xe8, 0xea, 0xee),
    },
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Tab {
    General,
    Hotkeys,
    About,
}

struct App {
    i18n: I18n,
    settings: Settings,
    dir: Option<PathBuf>,
    tab: Tab,
    logo: egui::TextureHandle,
    autostart: bool,
    recorder: hotkeys::Recorder,
    status: Option<String>,
}

fn theme_preference(theme: Theme) -> egui::ThemePreference {
    match theme {
        Theme::System => egui::ThemePreference::System,
        Theme::Light => egui::ThemePreference::Light,
        Theme::Dark => egui::ThemePreference::Dark,
    }
}

fn set_language(i18n: &mut I18n, code: &str) {
    let code = Some(code.to_string())
        .filter(|c| !c.is_empty())
        .or_else(kbg_platform::user_language)
        .unwrap_or_else(|| gob_i18n::DEFAULT_LANG.into());
    i18n.set_language(&code);
}

/// Debug builds open the tab named in `KEYBOARD_GOBLOON_TAB` (general | hotkeys | about), to
/// check a page by screenshot without clicking.
fn start_tab() -> Tab {
    let wanted = cfg!(debug_assertions)
        .then(|| std::env::var("KEYBOARD_GOBLOON_TAB").ok())
        .flatten();
    match wanted.as_deref() {
        Some("hotkeys") => Tab::Hotkeys,
        Some("about") => Tab::About,
        _ => Tab::General,
    }
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        let dir = kbg_settings::data_dir(kbg_platform::default_data_dir).map(Path::to_path_buf);
        let settings = dir.as_deref().map(Settings::load).unwrap_or_default();
        let lang_dir = std::env::current_exe()
            .ok()
            .and_then(|exe| exe.parent().map(|d| d.join("lang")));
        let mut i18n = I18n::new(LANGUAGES, lang_dir.as_deref());
        set_language(&mut i18n, &settings.language);
        let mut fonts = egui::FontDefinitions::default();
        gob_ui::add_icon_font(&mut fonts);
        cc.egui_ctx.set_fonts(fonts);
        gob_ui::apply(&cc.egui_ctx, &PALETTES, theme_preference(settings.theme));
        let logo = eframe::icon_data::from_png_bytes(LOGO_PNG).expect("embedded logo is a PNG");
        let logo = cc.egui_ctx.load_texture(
            "logo",
            egui::ColorImage::from_rgba_unmultiplied(
                [logo.width as usize, logo.height as usize],
                &logo.rgba,
            ),
            egui::TextureOptions::LINEAR,
        );
        Self {
            i18n,
            settings,
            dir,
            tab: start_tab(),
            logo,
            autostart: kbg_platform::autostart(),
            recorder: hotkeys::Recorder::default(),
            status: None,
        }
    }

    /// Writes settings.gob and tells the running app.
    fn save(&mut self) {
        let Some(dir) = &self.dir else { return };
        if let Err(e) = self.settings.save(dir) {
            self.status = Some(e.to_string());
            return;
        }
        let key = if kbg_platform::notify_app() {
            "status-saved"
        } else {
            "status-not-running"
        };
        self.status = Some(self.i18n.tr(key));
    }

    fn general(&mut self, ui: &mut egui::Ui) {
        let i18n = &self.i18n;
        section_title(ui, icon::POWER, &i18n.tr("general-startup"));
        if ui
            .add(gob_ui::switch(
                &mut self.autostart,
                i18n.tr("general-autostart"),
            ))
            .changed()
        {
            let app = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(|d| d.join(APP_EXE)));
            let done = app.is_some_and(|app| kbg_platform::set_autostart(&app, self.autostart));
            if !done {
                self.autostart = kbg_platform::autostart();
                self.status = Some(i18n.tr("status-autostart-failed"));
            }
        }
        ui.add_space(gob_ui::SPACE_L);

        section_title(ui, icon::PALETTE, &i18n.tr("general-appearance"));
        let mut changed = false;
        // Three theme buttons, as in Joystick Gobloon.
        ui.horizontal(|ui| {
            for (theme, glyph, key) in [
                (Theme::Light, icon::SUN, "theme-light"),
                (Theme::Dark, icon::MOON, "theme-dark"),
                (Theme::System, icon::MONITOR, "theme-system"),
            ] {
                let selected = self.settings.theme == theme;
                let text = RichText::new(format!("{glyph}  {}", i18n.tr(key)));
                let button = egui::Button::new(text)
                    .selected(selected)
                    .corner_radius(gob_ui::RADIUS)
                    .min_size(Vec2::new(96.0, 30.0));
                let tip = if theme == Theme::System {
                    i18n.tr("theme-system-tip")
                } else {
                    i18n.tr(key)
                };
                if ui.add(button).on_hover_text(tip).clicked() && !selected {
                    self.settings.theme = theme;
                    changed = true;
                }
            }
        });
        ui.add_space(gob_ui::SPACE_M);
        ui.horizontal(|ui| {
            ui.label(RichText::new(icon::GLOBE).size(gob_ui::ICON_SIZE))
                .on_hover_text(i18n.tr("general-language"));
            let system = i18n.tr("general-language-system");
            let languages = i18n.languages();
            let shown = languages
                .iter()
                .find(|(code, _)| *code == self.settings.language)
                .map_or(system.clone(), |(_, name)| name.clone());
            egui::ComboBox::from_id_salt("language")
                .selected_text(shown)
                .show_ui(ui, |ui| {
                    changed |= ui
                        .selectable_value(&mut self.settings.language, String::new(), system)
                        .changed();
                    for (code, name) in languages {
                        changed |= ui
                            .selectable_value(&mut self.settings.language, code, name)
                            .changed();
                    }
                });
        });
        if changed {
            set_language(&mut self.i18n, &self.settings.language);
            gob_ui::apply(ui.ctx(), &PALETTES, theme_preference(self.settings.theme));
            self.save();
        }
    }

    fn about(&self, ui: &mut egui::Ui) {
        let i18n = &self.i18n;
        let accent = gob_ui::accent(ui);
        ui.add_space(gob_ui::SPACE_M);
        ui.vertical_centered(|ui| {
            ui.set_max_width(440.0);
            ui.add(egui::Image::new(&self.logo).fit_to_exact_size(Vec2::splat(96.0)));
            ui.add_space(gob_ui::SPACE_S);
            ui.label(RichText::new(APP_NAME).size(24.0).strong());
            // Version chip; alpha builds (x.y.z-alpha.n) are marked.
            let alpha = VERSION.contains('-');
            let text = if alpha {
                format!("v{VERSION}  ·  {}", i18n.tr("about-alpha"))
            } else {
                format!("v{VERSION}")
            };
            let fill = if alpha {
                ui.visuals().warn_fg_color.gamma_multiply(0.18)
            } else {
                accent.gamma_multiply(0.18)
            };
            ui.add(
                egui::Button::new(RichText::new(text).small().strong())
                    .fill(fill)
                    .stroke(egui::Stroke::NONE)
                    .corner_radius(10)
                    .sense(egui::Sense::hover()),
            );
            ui.add_space(gob_ui::SPACE_M);
            ui.label(RichText::new(i18n.tr("about-text")).size(15.0));
            ui.add_space(gob_ui::SPACE_L);
            // Support: the one accented call to action.
            let support = egui::Button::new(
                RichText::new(format!("{}  {}", icon::HEART, i18n.tr("about-support")))
                    .size(15.0)
                    .color(ui.visuals().strong_text_color()),
            )
            .fill(accent.gamma_multiply(0.45))
            .stroke(egui::Stroke::new(1.0_f32, accent))
            .corner_radius(gob_ui::RADIUS)
            .min_size(Vec2::new(220.0, 36.0));
            if ui
                .add(support)
                .on_hover_text(i18n.tr("about-support-tip"))
                .clicked()
            {
                ui.ctx().open_url(egui::OpenUrl::new_tab(DONATE_URL));
            }
            ui.add_space(gob_ui::SPACE_M);
            ui.horizontal_wrapped(|ui| {
                let links = [
                    (icon::GLOBE, "about-website", WEBSITE.to_string()),
                    (icon::GITHUB_LOGO, "about-source", REPO.to_string()),
                    (icon::BUG, "about-issues", format!("{REPO}/issues")),
                ];
                for (glyph, key, url) in links {
                    ui.hyperlink_to(format!("{glyph}  {}", i18n.tr(key)), url);
                    ui.add_space(gob_ui::SPACE_M);
                }
            });
            ui.add_space(gob_ui::SPACE_L);
            ui.separator();
            ui.label(RichText::new(i18n.trf("about-created", &[("name", AUTHOR.into())])).weak());
            ui.label(RichText::new(i18n.tr("about-license")).weak());
        });
    }
}

fn section_title(ui: &mut egui::Ui, glyph: &str, title: &str) {
    ui.label(RichText::new(format!("{glyph}  {title}")).strong());
    ui.add_space(gob_ui::SPACE_XS);
}

impl eframe::App for App {
    fn ui(&mut self, root: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::bottom("status").show(root, |ui| {
            ui.add_space(gob_ui::SPACE_XS);
            ui.label(RichText::new(self.status.clone().unwrap_or_default()).weak());
        });
        egui::CentralPanel::default().show(root, |ui| {
            ui.horizontal(|ui| {
                for (tab, glyph, key) in [
                    (Tab::General, icon::SLIDERS_HORIZONTAL, "tab-general"),
                    (Tab::Hotkeys, icon::KEYBOARD, "tab-hotkeys"),
                    (Tab::About, icon::INFO, "tab-about"),
                ] {
                    if gob_ui::tab(ui, self.tab == tab, glyph, &self.i18n.tr(key)).clicked() {
                        self.tab = tab;
                    }
                }
            });
            ui.separator();
            ui.add_space(gob_ui::SPACE_S);
            match self.tab {
                Tab::General => self.general(ui),
                Tab::Hotkeys => {
                    if let Some(status) =
                        self.recorder
                            .show(ui, &self.i18n, &mut self.settings.hotkeys)
                    {
                        self.save();
                        if let Some(note) = status {
                            self.status = Some(note);
                        }
                    }
                }
                Tab::About => self.about(ui),
            }
        });
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        // The app's hotkeys come back even when the window closes mid-recording.
        self.recorder.stop();
    }
}

fn main() -> eframe::Result {
    // One settings window: a second start brings the first to the front.
    if kbg_platform::focus_window(APP_NAME) {
        return Ok(());
    }
    let icon = eframe::icon_data::from_png_bytes(LOGO_PNG).expect("embedded logo is a PNG");
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(APP_NAME)
            .with_inner_size([620.0, 520.0])
            .with_min_inner_size([520.0, 420.0])
            .with_icon(icon),
        ..Default::default()
    };
    eframe::run_native(APP_NAME, options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_language_has_every_key() {
        assert_eq!(gob_i18n::check_complete(super::LANGUAGES), Ok(()));
    }
}
