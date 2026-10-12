//! The debugger: starting a debug run, stepping, and the panel beside the
//! terminal that shows the variables, the calls and the loops of the
//! paused program.

use earth::debug::{Snapshot, Variable};
use eframe::egui::{self, Align, Layout, RichText, ScrollArea};

use super::{LineKind, StudioApp};
use crate::runner::Resume;
use crate::shortcuts::Action;

/// A program being debugged.
pub(super) struct DebugSession {
    /// The document the program came from.
    pub document: usize,
    /// Where it is paused, while it is.
    pub paused: Option<Snapshot>,
    /// The variables at the previous pause, to mark the ones that changed.
    pub previous: Vec<Variable>,
}

impl StudioApp {
    pub(super) fn start_debugging(&mut self) {
        if self.runner.is_busy() {
            return;
        }
        let breakpoints = self.documents[self.active].breakpoints.clone();
        self.runner.set_breakpoints(&breakpoints);
        self.debug = Some(DebugSession {
            document: self.active,
            paused: None,
            previous: Vec::new(),
        });
        let texts = self.texts();
        self.write_terminal(format!("🐞 {}\n", texts.debug_started), LineKind::Info);
        self.start_program(true);
    }

    pub(super) fn is_paused(&self) -> bool {
        self.debug
            .as_ref()
            .is_some_and(|session| session.paused.is_some())
    }

    pub(super) fn resume(&mut self, how: Resume) {
        if let Some(session) = &mut self.debug {
            if let Some(snapshot) = session.paused.take() {
                session.previous = snapshot.variables;
                self.runner.resume(how);
            }
        }
    }

    pub(super) fn paused_at(&mut self, snapshot: Snapshot) {
        let Some(session) = &mut self.debug else {
            return;
        };
        if snapshot.file.is_none() {
            self.active = session.document;
            self.scroll_to_line = Some(snapshot.line);
        }
        session.paused = Some(snapshot);
    }

    /// The buttons that drive a debug run, for the toolbar of the panel.
    fn debug_buttons(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let palette = self.palette();
        let paused = self.is_paused();
        let bindings = self.bindings.clone();
        let hint = |action: Action| crate::shortcuts::format(&bindings.get(action));
        let buttons = [
            (
                "▶",
                texts.debug_continue,
                Action::Continue,
                Resume::Continue,
            ),
            (
                "⏭",
                texts.debug_step_over,
                Action::StepOver,
                Resume::StepOver,
            ),
            (
                "⤵",
                texts.debug_step_into,
                Action::StepInto,
                Resume::StepInto,
            ),
            ("⤴", texts.debug_step_out, Action::StepOut, Resume::StepOut),
        ];
        for (icon, name, action, how) in buttons {
            let color = if action == Action::Continue {
                palette.success
            } else {
                palette.text
            };
            let button = egui::Button::new(RichText::new(icon).size(15.0).color(color))
                .min_size(egui::vec2(34.0, 26.0));
            if ui
                .add_enabled(paused, button)
                .on_hover_text(format!("{name} ({})", hint(action)))
                .clicked()
            {
                self.resume(how);
            }
        }
        let stop = egui::Button::new(RichText::new("⏹").size(15.0).color(palette.danger))
            .min_size(egui::vec2(34.0, 26.0));
        if ui
            .add(stop)
            .on_hover_text(format!("{} ({})", texts.stop_short, hint(Action::Stop)))
            .clicked()
        {
            self.runner.stop();
        }
    }

    pub(super) fn debug_panel(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let palette = self.palette();
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("🐞 {}", texts.debug_title)).strong());
            ui.add_space(6.0);
            self.debug_buttons(ui);
        });
        let state = match self.debug.as_ref().and_then(|s| s.paused.as_ref()) {
            Some(snapshot) => texts
                .debug_paused_at
                .replace("{line}", &snapshot.line.to_string()),
            None => texts.running.to_owned(),
        };
        ui.label(RichText::new(state).small().color(palette.warning));
        ui.separator();

        let Some(session) = &self.debug else {
            return;
        };
        let Some(snapshot) = &session.paused else {
            ui.label(RichText::new(texts.debug_running_hint).weak());
            return;
        };
        let changed = |variable: &Variable| {
            session
                .previous
                .iter()
                .find(|old| old.name == variable.name && old.global == variable.global)
                .is_none_or(|old| old.value != variable.value)
                && !session.previous.is_empty()
        };
        ScrollArea::vertical()
            .id_salt("debug")
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if let Some(file) = &snapshot.file {
                    ui.label(
                        RichText::new(texts.debug_in_module.replace("{file}", file))
                            .small()
                            .weak(),
                    );
                }
                egui::CollapsingHeader::new(RichText::new(texts.debug_variables).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        if snapshot.variables.is_empty() {
                            ui.label(RichText::new(texts.debug_no_variables).weak());
                        }
                        for (global, title) in
                            [(false, texts.debug_local), (true, texts.debug_global)]
                        {
                            let group: Vec<&Variable> = snapshot
                                .variables
                                .iter()
                                .filter(|variable| variable.global == global)
                                .collect();
                            if group.is_empty() {
                                continue;
                            }
                            ui.label(RichText::new(title).small().weak());
                            for (row, variable) in group.into_iter().enumerate() {
                                let value_color = if changed(variable) {
                                    palette.warning
                                } else {
                                    palette.syntax.number
                                };
                                let fill = if row % 2 == 1 {
                                    palette.button
                                } else {
                                    egui::Color32::TRANSPARENT
                                };
                                egui::Frame::new().fill(fill).show(ui, |ui| {
                                    ui.set_width(ui.available_width());
                                    ui.horizontal(|ui| {
                                        let row_height = 20.0;
                                        let name_width = 96.0;
                                        let type_width = 64.0;
                                        let left = Layout::left_to_right(Align::Center);
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(name_width, row_height),
                                            left,
                                            |ui| {
                                                ui.set_min_width(name_width);
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&variable.name)
                                                            .monospace()
                                                            .strong(),
                                                    )
                                                    .truncate(),
                                                );
                                            },
                                        );
                                        let value_width =
                                            (ui.available_width() - type_width).max(60.0);
                                        ui.allocate_ui_with_layout(
                                            egui::vec2(value_width, row_height),
                                            left,
                                            |ui| {
                                                ui.set_min_width(value_width);
                                                ui.add(
                                                    egui::Label::new(
                                                        RichText::new(&variable.value)
                                                            .monospace()
                                                            .color(value_color),
                                                    )
                                                    .truncate(),
                                                )
                                                .on_hover_text(&variable.value);
                                            },
                                        );
                                        ui.label(RichText::new(&variable.type_name).small().weak());
                                    });
                                });
                            }
                        }
                    });
                egui::CollapsingHeader::new(RichText::new(texts.debug_calls).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        for (depth, frame) in snapshot.frames.iter().enumerate().rev() {
                            let name = if frame.function.is_empty() {
                                texts.debug_program.to_owned()
                            } else {
                                format!("{}()", frame.function)
                            };
                            let line = texts.debug_line.replace("{line}", &frame.line.to_string());
                            let text = RichText::new(format!("{name} — {line}")).monospace();
                            let innermost = depth + 1 == snapshot.frames.len();
                            ui.label(if innermost {
                                text.color(palette.warning)
                            } else {
                                text
                            });
                        }
                    });
                egui::CollapsingHeader::new(RichText::new(texts.debug_loops).strong())
                    .default_open(true)
                    .show(ui, |ui| {
                        if snapshot.loops.is_empty() {
                            ui.label(RichText::new(texts.debug_no_loops).weak());
                        }
                        for turn in &snapshot.loops {
                            let text = match turn.total {
                                Some(total) => texts
                                    .debug_for_turn
                                    .replace("{line}", &turn.line.to_string())
                                    .replace("{turn}", &turn.turn.to_string())
                                    .replace("{total}", &total.to_string()),
                                None => texts
                                    .debug_while_turn
                                    .replace("{line}", &turn.line.to_string())
                                    .replace("{turn}", &turn.turn.to_string()),
                            };
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(text).monospace());
                                if let Some(total) = turn.total.filter(|total| *total > 0) {
                                    ui.add(
                                        egui::ProgressBar::new(turn.turn as f32 / total as f32)
                                            .desired_width(80.0)
                                            .desired_height(6.0)
                                            .fill(palette.accent),
                                    );
                                }
                            });
                        }
                    });
            });
    }
}
