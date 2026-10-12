//! The Earth Assistant panel on the right: setting up the AI service, the
//! chat, and applying code from answers to the editor.

use std::sync::{Arc, Mutex};

use eframe::egui::{
    self, Align, Color32, FontId, Key, Layout, Modifiers, RichText, ScrollArea, TextEdit,
};

use super::{Document, StudioApp};
use crate::assistant::{
    self, ChatError, Check, Entry, Job, OpenFile, Part, Phase, Request, Service,
};
use crate::credentials;
use crate::highlight::{self, DARK};
use crate::texts::Texts;

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

type Listing = Arc<Mutex<Option<Result<Vec<String>, ChatError>>>>;

/// The form for choosing the AI service, its key and model.
pub(super) struct Setup {
    service: Service,
    base_url: String,
    key: String,
    models: Vec<String>,
    model: String,
    listing: Option<Listing>,
    error: Option<ChatError>,
}

impl Setup {
    fn new(service: Service, base_url: &str, key: Option<&str>, model: &str) -> Self {
        let mut setup = Self {
            service,
            base_url: String::new(),
            key: key.unwrap_or_default().to_owned(),
            models: Vec::new(),
            model: model.to_owned(),
            listing: None,
            error: None,
        };
        setup.choose(service);
        if !base_url.is_empty() {
            setup.base_url = base_url.to_owned();
        }
        if !model.is_empty() {
            setup.models = vec![model.to_owned()];
        }
        setup
    }

    fn choose(&mut self, service: Service) {
        if service != self.service || self.base_url.is_empty() {
            self.base_url = service.base_url().unwrap_or_default().to_owned();
        }
        if service != self.service {
            self.key.clear();
            self.models.clear();
            self.model.clear();
            self.error = None;
        }
        self.service = service;
    }
}

/// The name of a service, as the settings store it.
pub(super) fn service_name(id: &str, texts: &Texts) -> &'static str {
    match Service::from_id(id) {
        Some(Service::Gemini) => "Google Gemini",
        Some(Service::OpenRouter) => "OpenRouter",
        Some(Service::OpenAI) => "OpenAI",
        Some(Service::Local) => texts.assistant_local_name,
        Some(Service::Custom) => texts.assistant_custom_name,
        None => texts.assistant_not_set,
    }
}

impl StudioApp {
    pub(super) fn toggle_assistant(&mut self) {
        self.settings.assistant_open = !self.settings.assistant_open;
        self.settings.save();
    }

    /// The service, address, key and model are all set.
    pub(super) fn assistant_ready(&self) -> bool {
        Service::from_id(&self.settings.ai_service).is_some_and(|service| {
            !self.settings.ai_model.is_empty()
                && !self.base_url().is_empty()
                && (!service.needs_key() || self.api_key.is_some())
        })
    }

    fn base_url(&self) -> String {
        if !self.settings.ai_base_url.is_empty() {
            return self.settings.ai_base_url.clone();
        }
        Service::from_id(&self.settings.ai_service)
            .and_then(Service::base_url)
            .unwrap_or_default()
            .to_owned()
    }

    pub(super) fn open_setup(&mut self) {
        if !self.settings.assistant_open {
            self.toggle_assistant();
        }
        let service = Service::from_id(&self.settings.ai_service).unwrap_or(Service::Gemini);
        self.setup = Some(Setup::new(
            service,
            &self.settings.ai_base_url,
            self.api_key.as_deref(),
            &self.settings.ai_model,
        ));
    }

    pub(super) fn forget_api_key(&mut self) {
        credentials::delete_api_key(&self.settings.ai_service);
        self.api_key = None;
        self.chat_job = None;
    }

    /// Opens the assistant and asks it about a program error.
    pub(super) fn explain_error(&mut self, error: &str) {
        if !self.settings.assistant_open {
            self.toggle_assistant();
        }
        if self.assistant_ready() && self.chat_job.is_none() {
            self.send_open_file = true;
            let prompt = self
                .texts()
                .assistant_explain_prompt
                .replace("{error}", error);
            self.send_chat(prompt);
        }
    }

    fn send_chat(&mut self, text: String) {
        if !self.assistant_ready() || self.chat_job.is_some() || text.trim().is_empty() {
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
                base_url: self.base_url(),
                api_key: self.api_key.clone(),
                model: self.settings.ai_model.clone(),
                system,
                history: self.chat.clone(),
            },
            Arc::clone(&self.wake),
        ));
    }

    /// Picks up finished answers and model lists.
    pub(super) fn poll_assistant(&mut self) {
        if let Some(job) = &self.chat_job {
            if let Some(result) = job.take_result() {
                self.chat_job = None;
                match result {
                    Ok(entry) => self.chat.push(entry),
                    Err(ChatError::Cancelled) => {}
                    Err(error) => self.chat_error = Some(error),
                }
            }
        }
        if let Some(setup) = &mut self.setup {
            let result = setup
                .listing
                .as_ref()
                .and_then(|listing| listing.lock().ok()?.take());
            if let Some(result) = result {
                setup.listing = None;
                match result {
                    Ok(models) if models.is_empty() => {
                        setup.error = Some(ChatError::Failed(String::new()));
                        setup.models.clear();
                    }
                    Ok(models) => {
                        setup.model = models[0].clone();
                        setup.models = models;
                        setup.error = None;
                    }
                    Err(error) => setup.error = Some(error),
                }
            }
        }
    }

    pub(super) fn assistant_panel(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let ready = self.assistant_ready();
        ui.horizontal(|ui| {
            ui.label(RichText::new(texts.assistant_title).small().strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button("×").clicked() {
                    self.toggle_assistant();
                }
            });
        });
        if (ready && self.setup.is_none()) || self.undo.is_some() {
            ui.horizontal_wrapped(|ui| {
                if ready && self.setup.is_none() {
                    if ui.small_button(texts.assistant_new_chat).clicked() {
                        self.chat_job = None;
                        self.chat.clear();
                        self.chat_error = None;
                    }
                    if ui.small_button(texts.assistant_service_button).clicked() {
                        self.open_setup();
                    }
                }
                if self.undo.is_some() && ui.small_button(texts.assistant_undo).clicked() {
                    self.undo_assistant_change();
                }
            });
        }
        ui.separator();

        if !ready && self.setup.is_none() {
            self.open_setup();
        }
        if self.setup.is_some() {
            ScrollArea::vertical()
                .id_salt("assistant_setup")
                .show(ui, |ui| self.setup_view(ui));
            return;
        }

        egui::Panel::bottom("assistant_input")
            .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6)))
            .show(ui, |ui| self.chat_input(ui));
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE)
            .show(ui, |ui| self.chat_view(ui));
    }

    fn setup_view(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let ready = self.assistant_ready();
        let wake = Arc::clone(&self.wake);
        let Some(setup) = &mut self.setup else {
            return;
        };
        ui.label(texts.assistant_intro);
        ui.add_space(8.0);
        ui.label(RichText::new(texts.assistant_setup_title).strong());
        for service in Service::ALL {
            let name = service_name(service.id(), texts);
            if ui.radio(setup.service == service, name).clicked() {
                setup.choose(service);
            }
            if setup.service == service {
                let note = match service {
                    Service::Gemini => texts.assistant_gemini_note,
                    Service::OpenRouter => texts.assistant_openrouter_note,
                    Service::OpenAI => texts.assistant_openai_note,
                    Service::Local => texts.assistant_local_note,
                    Service::Custom => texts.assistant_custom_note,
                };
                ui.indent(service.id(), |ui| {
                    ui.label(RichText::new(note).small().weak());
                });
            }
        }
        ui.add_space(8.0);

        if matches!(setup.service, Service::Local | Service::Custom) {
            ui.label(texts.assistant_address);
            ui.add(
                TextEdit::singleline(&mut setup.base_url)
                    .hint_text("http://localhost:11434/v1/")
                    .desired_width(f32::INFINITY),
            );
        }
        if setup.service != Service::Local {
            ui.horizontal(|ui| {
                ui.label(texts.assistant_key);
                if let Some(page) = setup.service.key_page() {
                    ui.hyperlink_to(texts.assistant_get_key, page);
                }
            });
            ui.add(
                TextEdit::singleline(&mut setup.key)
                    .password(true)
                    .desired_width(f32::INFINITY),
            );
            ui.label(RichText::new(texts.assistant_key_note).small().weak());
        }
        ui.add_space(6.0);

        let can_list = !setup.base_url.trim().is_empty()
            && (!setup.service.needs_key() || !setup.key.trim().is_empty());
        ui.horizontal(|ui| {
            if setup.listing.is_some() {
                ui.spinner();
                ui.label(texts.assistant_listing);
            } else if ui
                .add_enabled(can_list, egui::Button::new(texts.assistant_connect))
                .clicked()
            {
                let listing: Listing = Arc::new(Mutex::new(None));
                let target = Arc::clone(&listing);
                let (service, base_url) = (setup.service, setup.base_url.trim().to_owned());
                let key = Some(setup.key.trim().to_owned()).filter(|key| !key.is_empty());
                let _ = std::thread::Builder::new()
                    .name("earth-models".to_owned())
                    .spawn(move || {
                        let result = assistant::list_models(service, &base_url, key.as_deref());
                        if let Ok(mut slot) = target.lock() {
                            *slot = Some(result);
                        }
                        wake();
                    });
                setup.listing = Some(listing);
                setup.error = None;
            }
        });
        if let Some(error) = &setup.error {
            if *error == ChatError::Failed(String::new()) {
                ui.label(RichText::new(texts.assistant_no_models).color(RED));
            } else {
                error_view(ui, texts, error);
            }
        }

        let mut save = false;
        let mut cancel = false;
        if !setup.models.is_empty() {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(texts.assistant_model);
                egui::ComboBox::from_id_salt("assistant_model")
                    .selected_text(setup.model.as_str())
                    .width(220.0)
                    .show_ui(ui, |ui| {
                        for model in &setup.models {
                            ui.selectable_value(&mut setup.model, model.clone(), model);
                        }
                    });
            });
        }
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            save = ui
                .add_enabled(
                    !setup.model.is_empty() && can_list,
                    egui::Button::new(RichText::new(texts.assistant_save).color(GREEN)),
                )
                .clicked();
            if ready {
                cancel = ui.button(texts.cancel).clicked();
            }
        });

        if save {
            let Some(setup) = self.setup.take() else {
                return;
            };
            let id = setup.service.id().to_owned();
            let key = setup.key.trim().to_owned();
            if key.is_empty() {
                credentials::delete_api_key(&id);
                self.api_key = None;
            } else {
                credentials::save_api_key(&id, &key);
                self.api_key = Some(key);
            }
            let default_url = setup.service.base_url().unwrap_or_default();
            self.settings.ai_base_url = if setup.base_url.trim() == default_url {
                String::new()
            } else {
                setup.base_url.trim().to_owned()
            };
            self.settings.ai_service = id;
            self.settings.ai_model = setup.model;
            self.settings.save();
            self.chat_error = None;
        } else if cancel {
            self.setup = None;
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
                    error_view(ui, texts, error);
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
    texts: &Texts,
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
                                    &highlight::Marks::default(),
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

/// The error message, with the details GitHub or the system gave, which
/// can be selected and copied when reporting a problem.
fn error_view(ui: &mut egui::Ui, texts: &Texts, error: &ChatError) {
    ui.label(RichText::new(error_text(texts, error)).color(RED));
    if let ChatError::Network(detail) | ChatError::Failed(detail) = error {
        ui.add(
            egui::Label::new(
                RichText::new(format!("{}: {detail}", texts.assistant_detail))
                    .small()
                    .weak(),
            )
            .wrap()
            .selectable(true),
        );
    }
}

fn error_text(texts: &Texts, error: &ChatError) -> &'static str {
    match error {
        ChatError::InvalidKey => texts.assistant_invalid_key,
        ChatError::RateLimited => texts.assistant_rate_limited,
        ChatError::TooLong => texts.assistant_too_long,
        ChatError::UnknownModel => texts.assistant_unknown_model,
        ChatError::Network(_) | ChatError::Cancelled => texts.assistant_network,
        ChatError::Failed(_) => texts.assistant_failed,
    }
}

fn char_to_byte(text: &str, index: usize) -> usize {
    text.char_indices()
        .nth(index)
        .map_or(text.len(), |(byte, _)| byte)
}
