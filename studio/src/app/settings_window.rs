//! The settings window: language, theme and sizes, where the panels are,
//! keyboard shortcuts, the assistant, and about.

use earth::Lang;
use eframe::egui::{self, KeyboardShortcut, Modifiers, RichText};

use super::{assistant_panel, StudioApp};
use crate::settings::{self, Side};
use crate::shortcuts::{self, Action, Bindings};
use crate::theme::ThemeName;
use crate::update;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Tab {
    General,
    Layout,
    Shortcuts,
    Assistant,
    About,
}

impl StudioApp {
    pub(super) fn settings_window(&mut self, context: &egui::Context) {
        if !self.show_settings {
            self.capturing = None;
            return;
        }
        let texts = self.texts();
        let before = self.settings.clone();
        let mut open = true;
        let mut forget_key = false;
        let mut open_setup = false;
        egui::Window::new(format!("⚙ {}", texts.settings_title))
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(520.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(context, |ui| {
                ui.horizontal(|ui| {
                    for (tab, name) in [
                        (Tab::General, texts.settings_tab_general),
                        (Tab::Layout, texts.settings_tab_layout),
                        (Tab::Shortcuts, texts.settings_tab_shortcuts),
                        (Tab::Assistant, texts.settings_tab_assistant),
                        (Tab::About, texts.settings_about),
                    ] {
                        if ui
                            .selectable_label(self.settings_tab == tab, name)
                            .clicked()
                        {
                            self.settings_tab = tab;
                            self.capturing = None;
                        }
                    }
                });
                ui.separator();
                ui.set_min_height(300.0);
                match self.settings_tab {
                    Tab::General => self.general_settings(ui),
                    Tab::Layout => self.layout_settings(ui),
                    Tab::Shortcuts => self.shortcut_settings(ui),
                    Tab::Assistant => {
                        let (setup, forget) = self.assistant_settings(ui);
                        open_setup |= setup;
                        forget_key |= forget;
                    }
                    Tab::About => {
                        ui.label(
                            RichText::new(concat!("Earth Studio v", env!("CARGO_PKG_VERSION")))
                                .strong(),
                        );
                        ui.label(texts.about_text);
                        ui.hyperlink_to(texts.project_page, update::PROJECT_URL);
                        ui.add_space(8.0);
                        ui.checkbox(&mut self.settings.auto_update, texts.settings_auto_update);
                        if self.settings.auto_update != before.auto_update {
                            ui.label(RichText::new(texts.settings_restart_note).small().weak());
                        }
                    }
                }
            });
        if self.settings != before {
            self.settings.save();
        }
        if forget_key {
            self.forget_api_key();
        }
        if open_setup {
            self.open_setup();
            self.show_settings = false;
        }
        if !open {
            self.show_settings = false;
        }
    }

    fn general_settings(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        egui::Grid::new("general")
            .num_columns(2)
            .spacing([16.0, 12.0])
            .show(ui, |ui| {
                ui.label(texts.settings_language);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut self.settings.lang, Lang::Tr, "Türkçe");
                    ui.selectable_value(&mut self.settings.lang, Lang::En, "English");
                });
                ui.end_row();

                ui.label(texts.settings_theme);
                ui.horizontal(|ui| {
                    for (name, label) in [
                        (ThemeName::Midnight, texts.theme_midnight),
                        (ThemeName::Dark, texts.theme_dark),
                        (ThemeName::Light, texts.theme_light),
                    ] {
                        if ui
                            .selectable_label(self.settings.theme == name.id(), label)
                            .clicked()
                        {
                            self.settings.theme = name.id().to_owned();
                        }
                    }
                });
                ui.end_row();

                ui.label(texts.settings_font_size);
                ui.add(
                    egui::Slider::new(
                        &mut self.settings.font_size,
                        settings::MIN_FONT_SIZE..=settings::MAX_FONT_SIZE,
                    )
                    .step_by(1.0),
                );
                ui.end_row();

                ui.label(texts.settings_ui_scale);
                let mut percent = (self.settings.ui_scale * 100.0).round();
                let slider = egui::Slider::new(
                    &mut percent,
                    settings::MIN_UI_SCALE * 100.0..=settings::MAX_UI_SCALE * 100.0,
                )
                .step_by(10.0)
                .suffix("%");
                // Apply when the slider is let go, or the window jumps
                // under the pointer while dragging.
                let response = ui.add(slider);
                if response.drag_stopped() || (response.changed() && !response.dragged()) {
                    self.settings.ui_scale = percent / 100.0;
                }
                ui.end_row();
            });
    }

    fn layout_settings(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        egui::Grid::new("layout")
            .num_columns(2)
            .spacing([16.0, 12.0])
            .show(ui, |ui| {
                for (label, side) in [
                    (texts.layout_explorer, &mut self.settings.explorer_side),
                    (texts.layout_assistant, &mut self.settings.assistant_side),
                ] {
                    ui.label(label);
                    ui.horizontal(|ui| {
                        ui.selectable_value(side, Side::Left, texts.side_left);
                        ui.selectable_value(side, Side::Right, texts.side_right);
                    });
                    ui.end_row();
                }
            });
        ui.add_space(8.0);
        ui.label(RichText::new(texts.layout_hint).small().weak());
    }

    fn shortcut_settings(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        // A pressed key becomes the new shortcut of the action waiting for one.
        if let Some(action) = self.capturing {
            let pressed = ui.input(|input| {
                input.events.iter().find_map(|event| match event {
                    egui::Event::Key {
                        key,
                        pressed: true,
                        modifiers,
                        ..
                    } => Some((*key, *modifiers)),
                    _ => None,
                })
            });
            if let Some((key, modifiers)) = pressed {
                if key == egui::Key::Escape && modifiers == Modifiers::NONE {
                    self.capturing = None;
                } else {
                    let modifiers = Modifiers {
                        command: modifiers.command || modifiers.ctrl,
                        ctrl: false,
                        mac_cmd: false,
                        ..modifiers
                    };
                    self.bindings
                        .set(action, KeyboardShortcut::new(modifiers, key));
                    self.settings.shortcuts = self.bindings.to_settings();
                    self.capturing = None;
                }
            }
        }
        egui::Grid::new("shortcuts")
            .num_columns(3)
            .striped(true)
            .spacing([16.0, 6.0])
            .show(ui, |ui| {
                for action in Action::ALL {
                    ui.label(action_name(texts, action));
                    let shortcut = self.bindings.get(action);
                    let text = if self.capturing == Some(action) {
                        RichText::new(texts.shortcut_press_keys).italics()
                    } else {
                        RichText::new(shortcuts::format(&shortcut)).monospace()
                    };
                    if ui
                        .selectable_label(self.capturing == Some(action), text)
                        .on_hover_text(texts.shortcut_change_hint)
                        .clicked()
                    {
                        self.capturing = Some(action);
                    }
                    if let Some(other) = self.bindings.conflict(action, shortcut) {
                        ui.label(
                            RichText::new(
                                texts
                                    .shortcut_conflict
                                    .replace("{action}", action_name(texts, other)),
                            )
                            .small()
                            .color(self.palette().warning),
                        );
                    } else {
                        ui.label("");
                    }
                    ui.end_row();
                }
            });
        ui.add_space(6.0);
        if ui.button(texts.shortcut_reset).clicked() {
            self.bindings = Bindings::default();
            self.settings.shortcuts.clear();
            self.capturing = None;
        }
    }

    /// Returns (open the service set-up, forget the key).
    fn assistant_settings(&mut self, ui: &mut egui::Ui) -> (bool, bool) {
        let texts = self.texts();
        let mut open_setup = false;
        let mut forget_key = false;
        ui.horizontal(|ui| {
            ui.label(texts.assistant_settings_label);
            let current = if self.assistant_ready() {
                format!(
                    "{} · {}",
                    assistant_panel::service_name(self.settings.ai_service.as_str(), texts),
                    self.settings.ai_model
                )
            } else {
                texts.assistant_not_set.to_owned()
            };
            ui.label(RichText::new(current).weak());
            if ui.small_button(texts.assistant_change).clicked() {
                open_setup = true;
            }
        });
        if self.api_key.is_some() && ui.button(texts.assistant_forget_key).clicked() {
            forget_key = true;
        }
        (open_setup, forget_key)
    }
}

pub(super) fn action_name(texts: &crate::texts::Texts, action: Action) -> &'static str {
    match action {
        Action::Run => texts.action_run,
        Action::Debug => texts.action_debug,
        Action::Stop => texts.action_stop,
        Action::Continue => texts.debug_continue,
        Action::StepOver => texts.debug_step_over,
        Action::StepInto => texts.debug_step_into,
        Action::StepOut => texts.debug_step_out,
        Action::ToggleBreakpoint => texts.action_breakpoint,
        Action::Save => texts.action_save,
        Action::NewFile => texts.action_new_file,
        Action::Assistant => texts.action_assistant,
        Action::Complete => texts.action_complete,
    }
}
