//! The Earth Assistant panel on the right: connecting GitHub, the chat,
//! and applying code from answers to the editor.

use std::sync::Arc;

use eframe::egui::{
    self, Align, Color32, FontId, Key, Layout, Modifiers, RichText, ScrollArea, TextEdit,
};

use super::{Document, StudioApp};
use crate::assistant::{self, ChatError, Check, Entry, Job, OpenFile, Part, Phase, Request};
use crate::auth::{self, Failure, Provider, SignIn};
use crate::credentials;
use crate::highlight::{self, DARK};

/// Points the assistant at another server, for tests.
const URL_VARIABLE: &str = "EARTH_STUDIO_MODELS_URL";

const GREEN: Color32 = Color32::from_rgb(0x89, 0xd1, 0x85);
const RED: Color32 = Color32::from_rgb(0xf4, 0x87, 0x71);
const YELLOW: Color32 = Color32::from_rgb(0xdc, 0xdc, 0xaa);

/// Something to do with a code block, chosen in the panel.
enum CodeAction {
    Copy(String),
    Insert(String),
    Replace(String),
    NewFile(String),
}

impl StudioApp {
    pub(super) fn toggle_assistant(&mut self) {
        self.settings.assistant_open = !self.settings.assistant_open;
        self.settings.save();
    }

    pub(super) fn disconnect_github(&mut self) {
        credentials::delete();
        self.github_token = None;
        self.chat_job = None;
    }

    /// Opens the assistant and asks it about a program error.
    pub(super) fn explain_error(&mut self, error: &str) {
        if !self.settings.assistant_open {
            self.toggle_assistant();
        }
        if self.github_token.is_some() && self.chat_job.is_none() {
            self.send_open_file = true;
            let prompt = self
                .texts()
                .assistant_explain_prompt
                .replace("{error}", error);
            self.send_chat(prompt);
        }
    }

    fn send_chat(&mut self, text: String) {
        let Some(token) = self.github_token.clone() else {
            return;
        };
        if self.chat_job.is_some() || text.trim().is_empty() {
            return;
        }
        self.chat.push(Entry::user(text.trim().to_owned()));
        let document = &self.documents[self.active];
        let name = document.name(self.texts());
        let file = self.send_open_file.then(|| OpenFile {
            name: &name,
            text: &document.text,
        });
        let system = assistant::system_prompt(self.settings.lang, file);
        self.chat_error = None;
        self.chat_job = Some(Job::start(
            Request {
                endpoint: std::env::var(URL_VARIABLE)
                    .unwrap_or_else(|_| assistant::ENDPOINT.to_owned()),
                token,
                model: self.settings.assistant_model.clone(),
                system,
                history: self.chat.clone(),
            },
            Arc::clone(&self.wake),
        ));
    }

    /// Picks up finished answers and GitHub connections.
    pub(super) fn poll_assistant(&mut self) {
        if let Some(job) = &self.chat_job {
            if let Some(token) = job.take_refreshed_token() {
                credentials::save(&token);
                self.github_token = Some(token);
            }
            if let Some(result) = job.take_result() {
                self.chat_job = None;
                match result {
                    Ok(entry) => self.chat.push(entry),
                    Err(ChatError::Cancelled) => {}
                    Err(error) => {
                        if error == ChatError::SignInNeeded {
                            self.disconnect_github();
                        }
                        self.chat_error = Some(error);
                    }
                }
            }
        }

        let state = self.assistant_sign_in.as_ref().map(SignIn::state);
        if let Some(auth::State::Done(profile, token)) = state {
            self.assistant_sign_in = None;
            if let Some(token) = token {
                credentials::save(&token);
                self.github_token = Some(token);
                self.chat_error = None;
            }
            // Connecting GitHub also signs in, unless already signed in.
            if self.settings.profile.is_none() {
                self.settings.profile = Some(profile);
                self.settings.save();
                self.load_avatar();
            }
        }
    }

    pub(super) fn assistant_panel(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        ui.horizontal(|ui| {
            ui.label(RichText::new(texts.assistant_title).small().strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("×").clicked() {
                    self.toggle_assistant();
                }
                if self.github_token.is_some()
                    && ui.small_button(texts.assistant_new_chat).clicked()
                {
                    self.chat_job = None;
                    self.chat.clear();
                    self.chat_error = None;
                }
                if self.undo.is_some() && ui.small_button(texts.assistant_undo).clicked() {
                    self.undo_assistant_change();
                }
            });
        });
        ui.separator();

        if self.github_token.is_none() {
            self.connect_view(ui);
            return;
        }

        egui::Panel::bottom("assistant_input")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6)))
            .show(ui, |ui| self.chat_input(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.chat_view(ui));
    }

    fn connect_view(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        ui.label(texts.assistant_intro);
        ui.add_space(6.0);
        if let Some(error) = &self.chat_error {
            ui.label(RichText::new(error_text(texts, error)).color(RED));
            ui.add_space(6.0);
        }
        match self.assistant_sign_in.as_ref().map(SignIn::state) {
            None | Some(auth::State::Done(..)) => {
                ui.label(RichText::new(texts.assistant_connect_note).small().weak());
                ui.add_space(4.0);
                if ui
                    .add(
                        egui::Button::new(texts.assistant_connect)
                            .min_size(egui::vec2(200.0, 30.0)),
                    )
                    .clicked()
                {
                    self.assistant_sign_in =
                        Some(SignIn::start(Provider::GitHub, Arc::clone(&self.wake)));
                    self.assistant_browser_opened = false;
                }
            }
            Some(auth::State::Starting | auth::State::InBrowser { .. }) => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(texts.waiting);
                });
            }
            Some(auth::State::DeviceCode { user_code, url }) => {
                if !self.assistant_browser_opened {
                    self.assistant_browser_opened = true;
                    ui.ctx().copy_text(user_code.clone());
                    ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                }
                ui.label(texts.device_code_intro);
                ui.label(
                    RichText::new(&user_code)
                        .monospace()
                        .size(26.0)
                        .strong()
                        .color(GREEN),
                );
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(RichText::new(texts.waiting).small());
                });
                ui.horizontal(|ui| {
                    if ui.button(texts.copy_code).clicked() {
                        ui.ctx().copy_text(user_code.clone());
                    }
                    if ui.button(texts.open_page).clicked() {
                        ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                    }
                    if ui.button(texts.cancel).clicked() {
                        self.assistant_sign_in = None;
                    }
                });
            }
            Some(auth::State::Failed(failure)) => {
                let message = match failure {
                    Failure::Denied => texts.failure_denied,
                    Failure::TimedOut => texts.failure_timed_out,
                    Failure::Network => texts.failure_network,
                    Failure::DeviceFlowDisabled => texts.failure_device_flow,
                };
                ui.label(RichText::new(message).color(RED));
                if ui.button(texts.cancel).clicked() {
                    self.assistant_sign_in = None;
                }
            }
        }
    }

    fn chat_view(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let font_size = self.settings.font_size - 1.0;
        let mut actions = Vec::new();
        let mut suggestion = None;
        ScrollArea::vertical()
            .id_salt("chat")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if self.chat.is_empty() && self.chat_job.is_none() {
                    ui.label(RichText::new(texts.assistant_intro).weak());
                    ui.add_space(8.0);
                    for prompt in [
                        texts.assistant_suggest_explain,
                        texts.assistant_suggest_write,
                        texts.assistant_suggest_improve,
                    ] {
                        if ui.button(format!("💬 {prompt}")).clicked() {
                            suggestion = Some(prompt.to_owned());
                        }
                    }
                }
                for (index, entry) in self.chat.iter().enumerate() {
                    match entry.role {
                        assistant::Role::User => user_bubble(ui, &entry.text),
                        assistant::Role::Assistant => {
                            show_answer(
                                ui,
                                &entry.text,
                                font_size,
                                index,
                                Some(&mut actions),
                                texts,
                            );
                            match entry.check {
                                Check::Passed => {
                                    ui.label(
                                        RichText::new(texts.assistant_checked).small().color(GREEN),
                                    );
                                }
                                Check::Failed => {
                                    ui.label(
                                        RichText::new(texts.assistant_unchecked)
                                            .small()
                                            .color(YELLOW),
                                    );
                                }
                                Check::NoCode => {}
                            }
                        }
                    }
                    ui.add_space(10.0);
                }
                if let Some(job) = &self.chat_job {
                    let (text, phase) = job.progress();
                    show_answer(ui, &text, font_size, usize::MAX, None, texts);
                    ui.horizontal(|ui| {
                        ui.spinner();
                        ui.label(
                            RichText::new(match phase {
                                Phase::Writing => texts.assistant_writing,
                                Phase::Fixing => texts.assistant_fixing,
                            })
                            .small(),
                        );
                    });
                }
                if let Some(error) = &self.chat_error {
                    ui.label(RichText::new(error_text(texts, error)).color(RED));
                }
            });

        if let Some(prompt) = suggestion {
            self.send_chat(prompt);
        }
        for action in actions {
            self.apply(ui.ctx(), action);
        }
    }

    fn chat_input(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let busy = self.chat_job.is_some();
        let id = ui.make_persistent_id("assistant_text");
        // Enter sends; Shift+Enter starts a new line.
        let enter = ui.memory(|memory| memory.has_focus(id))
            && ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Enter));
        ui.add(
            TextEdit::multiline(&mut self.chat_input)
                .id(id)
                .hint_text(texts.assistant_placeholder)
                .desired_rows(3)
                .desired_width(f32::INFINITY),
        );
        let mut send = enter && !busy;
        ui.horizontal(|ui| {
            ui.checkbox(&mut self.send_open_file, texts.assistant_include_file);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if busy {
                    if ui.button(texts.assistant_stop).clicked() {
                        self.chat_job = None;
                    }
                } else {
                    send |= ui
                        .add_enabled(
                            !self.chat_input.trim().is_empty(),
                            egui::Button::new(RichText::new(texts.assistant_send).color(GREEN)),
                        )
                        .clicked();
                }
            });
        });
        if send && !self.chat_input.trim().is_empty() {
            let text = std::mem::take(&mut self.chat_input);
            self.send_chat(text);
        }
    }

    fn apply(&mut self, context: &egui::Context, action: CodeAction) {
        match action {
            CodeAction::Copy(code) => context.copy_text(code),
            CodeAction::Insert(code) => {
                let active = self.active;
                let document = &mut self.documents[active];
                self.undo = Some((active, document.text.clone()));
                let at = self
                    .cursor_index
                    .map(|index| char_to_byte(&document.text, index))
                    .unwrap_or(document.text.len());
                let mut code = code;
                if !code.ends_with('\n') {
                    code.push('\n');
                }
                if at > 0 && !document.text[..at].ends_with('\n') {
                    code.insert(0, '\n');
                }
                document.text.insert_str(at, &code);
                document.error_line = None;
            }
            CodeAction::Replace(code) => self.replace_preview = Some(code),
            CodeAction::NewFile(code) => {
                self.new_document();
                let document = &mut self.documents[self.active];
                document.text = format!("{code}\n");
            }
        }
    }

    fn undo_assistant_change(&mut self) {
        if let Some((index, text)) = self.undo.take() {
            if let Some(document) = self.documents.get_mut(index) {
                document.text = text;
                self.active = index;
            }
        }
    }

    /// Shows what "Replace file" will change, and applies it on approval.
    pub(super) fn replace_window(&mut self, context: &egui::Context) {
        let Some(new_text) = &self.replace_preview else {
            return;
        };
        let texts = self.texts();
        let old_text = &self.documents[self.active].text;
        let font = FontId::monospace(self.settings.font_size - 2.0);
        let mut choice = None;
        egui::Window::new(texts.assistant_replace_title)
            .collapsible(false)
            .resizable(true)
            .default_width(620.0)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(context, |ui| {
                ui.label(texts.assistant_replace_body);
                ui.add_space(4.0);
                ScrollArea::both().max_height(420.0).show(ui, |ui| {
                    for line in assistant::diff(old_text, new_text) {
                        let (prefix, text, color) = match line {
                            assistant::DiffLine::Same(text) => {
                                (" ", text, Color32::from_gray(0x9a))
                            }
                            assistant::DiffLine::Removed(text) => ("-", text, RED),
                            assistant::DiffLine::Added(text) => ("+", text, GREEN),
                        };
                        ui.label(
                            RichText::new(format!("{prefix} {text}"))
                                .font(font.clone())
                                .color(color),
                        );
                    }
                });
                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button(texts.assistant_apply).clicked() {
                        choice = Some(true);
                    }
                    if ui.button(texts.cancel).clicked() {
                        choice = Some(false);
                    }
                });
            });
        match choice {
            Some(true) => {
                let new_text = self.replace_preview.take().unwrap_or_default();
                let active = self.active;
                let document: &mut Document = &mut self.documents[active];
                self.undo = Some((
                    active,
                    std::mem::replace(&mut document.text, new_text + "\n"),
                ));
                document.error_line = None;
            }
            Some(false) => self.replace_preview = None,
            None => {}
        }
    }
}

fn user_bubble(ui: &mut egui::Ui, text: &str) {
    egui::Frame::new()
        .fill(Color32::from_rgb(0x26, 0x3a, 0x52))
        .corner_radius(6.0)
        .inner_margin(egui::Margin::symmetric(10, 6))
        .show(ui, |ui| {
            ui.add(egui::Label::new(text).wrap().selectable(true));
        });
}

/// Draws an answer: prose as text, code highlighted, with buttons when
/// `actions` is given (not while the answer is still being written).
fn show_answer(
    ui: &mut egui::Ui,
    text: &str,
    font_size: f32,
    index: usize,
    mut actions: Option<&mut Vec<CodeAction>>,
    texts: &crate::texts::Texts,
) {
    for (part_index, part) in assistant::parts(text).into_iter().enumerate() {
        match part {
            Part::Text(text) => {
                ui.add(
                    egui::Label::new(prose(text, font_size + 0.5))
                        .wrap()
                        .selectable(true),
                );
            }
            Part::Code(code) => {
                egui::Frame::new()
                    .fill(Color32::from_rgb(0x14, 0x14, 0x14))
                    .corner_radius(4.0)
                    .inner_margin(egui::Margin::same(8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ScrollArea::horizontal()
                            .id_salt(("code", index, part_index))
                            .show(ui, |ui| {
                                let job = highlight::layout(
                                    &code,
                                    FontId::monospace(font_size),
                                    &DARK,
                                    None,
                                );
                                ui.add(egui::Label::new(job).selectable(true).extend());
                            });
                    });
                if let Some(actions) = actions.as_deref_mut() {
                    ui.horizontal_wrapped(|ui| {
                        if ui.small_button(texts.assistant_copy).clicked() {
                            actions.push(CodeAction::Copy(code.clone()));
                        }
                        if ui.small_button(texts.assistant_insert).clicked() {
                            actions.push(CodeAction::Insert(code.clone()));
                        }
                        if ui.small_button(texts.assistant_replace).clicked() {
                            actions.push(CodeAction::Replace(code.clone()));
                        }
                        if ui.small_button(texts.assistant_new_file).clicked() {
                            actions.push(CodeAction::NewFile(code.clone()));
                        }
                    });
                }
            }
        }
    }
}

/// Prose with the bits of Markdown models use: `code`, **bold**, headings
/// and list items.
fn prose(text: &str, size: f32) -> egui::text::LayoutJob {
    let mut job = egui::text::LayoutJob::default();
    let plain = egui::TextFormat {
        font_id: FontId::proportional(size),
        color: Color32::from_gray(0xd4),
        ..Default::default()
    };
    let strong = egui::TextFormat {
        color: Color32::WHITE,
        ..plain.clone()
    };
    let code = egui::TextFormat {
        font_id: FontId::monospace(size - 1.0),
        color: YELLOW,
        background: Color32::from_rgb(0x2a, 0x2a, 0x2a),
        ..Default::default()
    };
    for (index, line) in text.lines().enumerate() {
        if index > 0 {
            job.append("\n", 0.0, plain.clone());
        }
        let trimmed = line.trim_start();
        let (line, base) = if let Some(heading) = trimmed.strip_prefix('#') {
            (heading.trim_start_matches('#').trim_start(), &strong)
        } else if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            job.append("  • ", 0.0, plain.clone());
            (item, &plain)
        } else {
            (line, &plain)
        };
        // Split on ` first, then on ** inside the non-code pieces.
        for (piece_index, piece) in line.split('`').enumerate() {
            if piece_index % 2 == 1 {
                job.append(piece, 0.0, code.clone());
                continue;
            }
            for (bold_index, bold) in piece.split("**").enumerate() {
                let format = if bold_index % 2 == 1 { &strong } else { base };
                job.append(bold, 0.0, format.clone());
            }
        }
    }
    job
}

fn error_text(texts: &crate::texts::Texts, error: &ChatError) -> &'static str {
    match error {
        ChatError::SignInNeeded => texts.assistant_sign_in_needed,
        ChatError::NoAccess => texts.assistant_no_access,
        ChatError::RateLimited => texts.assistant_rate_limited,
        ChatError::TooLong => texts.assistant_too_long,
        ChatError::UnknownModel => texts.assistant_unknown_model,
        ChatError::Network | ChatError::Cancelled => texts.assistant_network,
    }
}

fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(byte, _)| byte)
}
