//! The Earth Studio window: explorer on the left, tabbed editor in the
//! middle, terminal at the bottom.

use std::fs;
use std::path::{Path, PathBuf};

use earth::Lang;
use eframe::egui::{
    self, text::LayoutJob, Align, Color32, FontId, Key, KeyboardShortcut, Layout, Modifiers,
    RichText, ScrollArea, TextEdit, TextFormat, ViewportCommand,
};

use crate::files::{self, Node};
use crate::highlight::{self, DARK};
use crate::runner::{Event, Runner};
use crate::texts::{texts, Texts};

/// Older terminal output is dropped beyond this many characters.
const TERMINAL_LIMIT: usize = 200_000;
const EDITOR_FONT_SIZE: f32 = 15.0;

const RUN: KeyboardShortcut = KeyboardShortcut::new(Modifiers::NONE, Key::F5);
const STOP: KeyboardShortcut = KeyboardShortcut::new(Modifiers::SHIFT, Key::F5);
const SAVE: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::S);
const NEW: KeyboardShortcut = KeyboardShortcut::new(Modifiers::COMMAND, Key::N);

struct Document {
    path: Option<PathBuf>,
    text: String,
    saved_text: String,
    /// The line of the last error reported in this file, 1-based.
    error_line: Option<usize>,
}

impl Document {
    fn name(&self, texts: &Texts) -> String {
        self.path
            .as_ref()
            .and_then(|path| path.file_name())
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| texts.untitled.to_owned())
    }

    /// The tab title: the name, with a dot while there are unsaved changes.
    fn title(&self, texts: &Texts) -> String {
        if self.is_dirty() {
            format!("{} •", self.name(texts))
        } else {
            self.name(texts)
        }
    }

    fn is_dirty(&self) -> bool {
        self.text != self.saved_text
    }
}

#[derive(Clone, Copy, PartialEq)]
enum LineKind {
    Output,
    Error,
    Info,
    Echo,
}

enum Confirm {
    Quit,
    CloseTab(usize),
}

pub struct StudioApp {
    lang: Lang,
    folder: Option<PathBuf>,
    tree: Vec<Node>,
    documents: Vec<Document>,
    active: usize,
    runner: Runner,
    /// Terminal contents as coloured pieces of text.
    terminal: Vec<(String, LineKind)>,
    terminal_input: String,
    focus_terminal: bool,
    waiting_for_input: bool,
    /// Whether the runner is busy with a program (not a REPL line).
    running_program: bool,
    /// The document the running program came from.
    running_document: Option<usize>,
    status: Option<String>,
    new_file_name: Option<String>,
    confirm: Option<Confirm>,
    allow_close: bool,
    cursor: (usize, usize),
    logo: Option<egui::TextureHandle>,
}

impl StudioApp {
    pub fn new(context: &eframe::CreationContext<'_>, start: Option<PathBuf>) -> Self {
        let egui_context = context.egui_ctx.clone();
        configure_style(&egui_context);
        let lang = Lang::Tr;
        let mut app = Self {
            lang,
            folder: None,
            tree: Vec::new(),
            documents: Vec::new(),
            active: 0,
            runner: Runner::new(move || egui_context.request_repaint()),
            terminal: Vec::new(),
            terminal_input: String::new(),
            focus_terminal: false,
            waiting_for_input: false,
            running_program: false,
            running_document: None,
            status: None,
            new_file_name: None,
            confirm: None,
            allow_close: false,
            cursor: (1, 1),
            logo: load_logo(&context.egui_ctx),
        };

        // `earth-studio program.earth` may give a relative path.
        let start = start.map(|path| std::path::absolute(&path).unwrap_or(path));
        match start {
            Some(path) if path.is_dir() => app.open_folder(path),
            Some(path) => {
                if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
                    app.open_folder(parent.to_path_buf());
                }
                app.open_file(&path);
            }
            None => {}
        }
        if app.documents.is_empty() {
            let welcome = texts(lang).welcome.to_owned();
            app.documents.push(Document {
                path: None,
                saved_text: welcome.clone(),
                text: welcome,
                error_line: None,
            });
        }
        app
    }

    fn texts(&self) -> &'static Texts {
        texts(self.lang)
    }

    // ----- files -----

    fn open_folder(&mut self, folder: PathBuf) {
        self.tree = files::scan(&folder);
        self.folder = Some(folder);
    }

    fn refresh_tree(&mut self) {
        if let Some(folder) = &self.folder {
            self.tree = files::scan(folder);
        }
    }

    fn open_file(&mut self, path: &Path) {
        if let Some(index) = self
            .documents
            .iter()
            .position(|document| document.path.as_deref() == Some(path))
        {
            self.active = index;
            return;
        }
        match fs::read_to_string(path) {
            Ok(text) => {
                // Replace the untouched welcome tab instead of piling up tabs.
                if self.documents.len() == 1
                    && self.documents[0].path.is_none()
                    && !self.documents[0].is_dirty()
                {
                    self.documents.clear();
                }
                self.documents.push(Document {
                    path: Some(path.to_path_buf()),
                    saved_text: text.clone(),
                    text,
                    error_line: None,
                });
                self.active = self.documents.len() - 1;
            }
            Err(error) => {
                self.status = Some(format!(
                    "{}: {} ({error})",
                    self.texts().could_not_open,
                    path.display()
                ));
            }
        }
    }

    fn new_document(&mut self) {
        self.documents.push(Document {
            path: None,
            text: String::new(),
            saved_text: String::new(),
            error_line: None,
        });
        self.active = self.documents.len() - 1;
    }

    /// Saves a document, asking for a file name if it has none. Returns
    /// whether it was saved.
    fn save(&mut self, index: usize) -> bool {
        let texts = self.texts();
        let path = match self.documents[index].path.clone() {
            Some(path) => path,
            None => {
                let mut dialog = rfd::FileDialog::new()
                    .add_filter(texts.earth_files, &["earth"])
                    .set_file_name("program.earth");
                if let Some(folder) = &self.folder {
                    dialog = dialog.set_directory(folder);
                }
                match dialog.save_file() {
                    Some(path) => path,
                    None => return false,
                }
            }
        };
        let document = &mut self.documents[index];
        match fs::write(&path, &document.text) {
            Ok(()) => {
                document.saved_text = document.text.clone();
                document.path = Some(path);
                self.status = Some(texts.saved.to_owned());
                self.refresh_tree();
                true
            }
            Err(error) => {
                self.status = Some(format!(
                    "{}: {} ({error})",
                    texts.could_not_save,
                    path.display()
                ));
                false
            }
        }
    }

    /// Saves every changed document that already has a file.
    fn save_all_named(&mut self) {
        for index in 0..self.documents.len() {
            if self.documents[index].path.is_some() && self.documents[index].is_dirty() {
                self.save(index);
            }
        }
    }

    fn create_file_in_folder(&mut self, name: &str) {
        let Some(folder) = self.folder.clone() else {
            return;
        };
        let path = folder.join(files::with_earth_extension(name));
        if path.exists() {
            self.status = Some(format!(
                "{}: {}",
                self.texts().already_exists,
                path.display()
            ));
            return;
        }
        match fs::write(&path, "") {
            Ok(()) => {
                self.refresh_tree();
                self.open_file(&path);
            }
            Err(error) => {
                self.status = Some(format!("{}: {error}", self.texts().could_not_save));
            }
        }
    }

    fn close_tab(&mut self, index: usize) {
        self.documents.remove(index);
        if self.documents.is_empty() {
            self.new_document();
        }
        if self.active >= self.documents.len() {
            self.active = self.documents.len() - 1;
        }
        self.running_document = None;
    }

    // ----- running -----

    fn run(&mut self) {
        if self.runner.is_busy() {
            return;
        }
        self.save_all_named();
        for document in &mut self.documents {
            document.error_line = None;
        }
        let document = &self.documents[self.active];
        let base_dir = document
            .path
            .as_ref()
            .and_then(|path| path.parent().map(Path::to_path_buf))
            .or_else(|| self.folder.clone())
            .unwrap_or_else(|| PathBuf::from("."));
        let title = document.name(self.texts());
        let (source, file) = (document.text.clone(), document.path.clone());

        self.write_terminal(format!("▶ {title}\n"), LineKind::Info);
        self.running_program = true;
        self.running_document = Some(self.active);
        self.runner.run(source, base_dir, file);
    }

    fn submit_terminal_line(&mut self) {
        let line = std::mem::take(&mut self.terminal_input);
        if self.runner.is_busy() {
            if self.running_program {
                self.write_terminal(format!("{line}\n"), LineKind::Echo);
                self.runner.send_input(line);
                self.waiting_for_input = false;
            }
        } else if !line.trim().is_empty() {
            self.write_terminal(format!("› {line}\n"), LineKind::Echo);
            self.running_program = false;
            self.runner.repl(line, self.lang);
        }
        self.focus_terminal = true;
    }

    fn handle_events(&mut self) {
        for event in self.runner.poll() {
            match event {
                Event::Output(text) => self.write_terminal(text, LineKind::Output),
                Event::Value(text) => self.write_terminal(format!("{text}\n"), LineKind::Output),
                Event::WaitingForInput => {
                    self.waiting_for_input = true;
                    self.focus_terminal = true;
                }
                Event::Error {
                    text,
                    line,
                    column,
                    file,
                } => self.show_error(text, line, column, file),
                Event::Finished { interrupted } => {
                    self.waiting_for_input = false;
                    if self.running_program {
                        let texts = self.texts();
                        let message = if interrupted {
                            texts.stopped
                        } else {
                            texts.finished
                        };
                        self.write_terminal(format!("■ {message}\n"), LineKind::Info);
                    }
                    self.running_program = false;
                }
            }
        }
    }

    fn show_error(
        &mut self,
        text: String,
        line: Option<usize>,
        column: Option<usize>,
        file: Option<PathBuf>,
    ) {
        // Find the document the error belongs to, opening it if needed. A
        // REPL line has no document.
        let document = match &file {
            _ if !self.running_program => None,
            Some(path) => {
                if !self.documents.iter().any(|d| d.path.as_ref() == Some(path)) {
                    self.open_file(path);
                }
                self.documents
                    .iter()
                    .position(|document| document.path.as_ref() == Some(path))
            }
            None => self.running_document,
        };
        let place = match (&file, line, column) {
            (Some(path), Some(line), Some(column)) => format!(
                "{}:{line}:{column}: ",
                path.file_name().unwrap_or_default().to_string_lossy()
            ),
            (None, Some(line), Some(column)) => format!("{line}:{column}: "),
            _ => String::new(),
        };
        let prefix = self.texts().error_in;
        self.write_terminal(format!("✖ {prefix}: {place}{text}\n"), LineKind::Error);
        if let (Some(index), Some(line)) = (document, line) {
            self.documents[index].error_line = Some(line);
            if self.running_program {
                self.active = index;
            }
        }
    }

    fn write_terminal(&mut self, text: String, kind: LineKind) {
        match self.terminal.last_mut() {
            Some((last, last_kind)) if *last_kind == kind => last.push_str(&text),
            _ => self.terminal.push((text, kind)),
        }
        let mut total: usize = self.terminal.iter().map(|(text, _)| text.len()).sum();
        while total > TERMINAL_LIMIT && self.terminal.len() > 1 {
            total -= self.terminal.remove(0).0.len();
        }
    }

    // ----- drawing -----

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        ui.horizontal(|ui| {
            if let Some(logo) = &self.logo {
                ui.add(egui::Image::new(logo).fit_to_exact_size(egui::vec2(24.0, 24.0)));
            }
            ui.label(RichText::new("Earth Studio").strong().size(16.0));
            ui.separator();
            if ui.button(texts.open_folder).clicked() {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    self.open_folder(folder);
                }
            }
            if ui
                .button(texts.new_file)
                .on_hover_text(ui.ctx().format_shortcut(&NEW))
                .clicked()
            {
                self.new_document();
            }
            if ui
                .button(texts.save)
                .on_hover_text(ui.ctx().format_shortcut(&SAVE))
                .clicked()
            {
                self.save(self.active);
            }
            ui.separator();
            let busy = self.runner.is_busy();
            let run = egui::Button::new(
                RichText::new(texts.run).color(Color32::from_rgb(0x89, 0xd1, 0x85)),
            );
            if ui.add_enabled(!busy, run).on_hover_text("F5").clicked() {
                self.run();
            }
            let stop = egui::Button::new(
                RichText::new(texts.stop).color(Color32::from_rgb(0xf4, 0x87, 0x71)),
            );
            if ui
                .add_enabled(busy, stop)
                .on_hover_text("Shift+F5")
                .clicked()
            {
                self.runner.stop();
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let other = match self.lang {
                    Lang::Tr => (Lang::En, "English"),
                    Lang::En => (Lang::Tr, "Türkçe"),
                };
                if ui.button(format!("🌐 {}", other.1)).clicked() {
                    self.lang = other.0;
                }
            });
        });
    }

    fn explorer(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        ui.horizontal(|ui| {
            ui.label(RichText::new(texts.explorer).small().strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.folder.is_some() {
                    if ui.small_button(texts.refresh).clicked() {
                        self.refresh_tree();
                    }
                    if ui.small_button(texts.new_file_in_folder).clicked() {
                        self.new_file_name = Some(String::new());
                    }
                }
            });
        });
        ui.separator();

        if let Some(name) = &mut self.new_file_name {
            let response = ui.add(TextEdit::singleline(name).hint_text(texts.file_name_hint));
            response.request_focus();
            let submitted = response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter));
            let mut create = submitted;
            let mut cancel = ui.input(|i| i.key_pressed(Key::Escape));
            ui.horizontal(|ui| {
                create |= ui.button(texts.create).clicked();
                cancel |= ui.button(texts.cancel).clicked();
            });
            if create && !name.trim().is_empty() {
                let name = name.clone();
                self.new_file_name = None;
                self.create_file_in_folder(&name);
            } else if cancel {
                self.new_file_name = None;
            }
            ui.separator();
        }

        let Some(folder) = &self.folder else {
            ui.label(texts.no_folder);
            if ui.button(texts.open_folder).clicked() {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    self.open_folder(folder);
                }
            }
            return;
        };
        let folder_name = folder
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_else(|| folder.display().to_string());
        ui.label(RichText::new(folder_name.to_uppercase()).strong());

        let active_path = self.documents[self.active].path.clone();
        let mut clicked = None;
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                show_nodes(ui, &self.tree, active_path.as_deref(), &mut clicked);
            });
        if let Some(path) = clicked {
            self.open_file(&path);
        }
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let mut close = None;
        ScrollArea::horizontal().id_salt("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, document) in self.documents.iter().enumerate() {
                    let selected = index == self.active;
                    let title = RichText::new(document.title(texts));
                    let title = if selected { title.strong() } else { title };
                    if ui.selectable_label(selected, title).clicked() {
                        self.active = index;
                    }
                    if ui.small_button("×").clicked() {
                        close = Some(index);
                    }
                    ui.add_space(6.0);
                }
            });
        });
        if let Some(index) = close {
            if self.documents[index].is_dirty() {
                self.confirm = Some(Confirm::CloseTab(index));
            } else {
                self.close_tab(index);
            }
        }
    }

    fn editor(&mut self, ui: &mut egui::Ui) {
        let font = FontId::monospace(EDITOR_FONT_SIZE);
        let document = &mut self.documents[self.active];
        let error_line = document.error_line;
        let line_count = document.text.split('\n').count();

        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap_width: f32| {
            // No wrapping: each source line stays one row, matching the gutter.
            let job = highlight::layout(text.as_str(), font.clone(), &DARK, error_line);
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };

        let mut changed = false;
        let mut cursor = None;
        ScrollArea::both()
            .id_salt(("editor", self.active))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.add(
                        egui::Label::new(line_numbers(line_count, error_line, &font))
                            .selectable(false),
                    );
                    let output = TextEdit::multiline(&mut document.text)
                        .id_salt(("text", self.active))
                        .font(font.clone())
                        .code_editor()
                        .frame(egui::Frame::NONE)
                        .lock_focus(true)
                        .desired_width(f32::INFINITY)
                        .desired_rows(30)
                        .layouter(&mut layouter)
                        .show(ui);
                    changed = output.response.changed();
                    cursor = output.cursor_range.map(|range| range.primary.index.0);
                });
            });

        if changed {
            document.error_line = None;
        }
        if let Some(index) = cursor {
            self.cursor = line_and_column(&document.text, index);
        }
    }

    fn terminal(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        // Fill the panel, or the resizable panel shrinks to its content.
        ui.set_min_height(ui.available_height());
        ui.horizontal(|ui| {
            ui.label(RichText::new(texts.terminal).small().strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button(texts.clear).clicked() {
                    self.terminal.clear();
                }
            });
        });
        ui.separator();

        let font = FontId::monospace(EDITOR_FONT_SIZE - 1.0);
        let input_height = 36.0;
        ScrollArea::vertical()
            .id_salt("terminal")
            .stick_to_bottom(true)
            .auto_shrink([false, false])
            .max_height(ui.available_height() - input_height)
            .show(ui, |ui| {
                let mut job = LayoutJob::default();
                for (text, kind) in &self.terminal {
                    let color = match kind {
                        LineKind::Output => DARK.text,
                        LineKind::Error => Color32::from_rgb(0xf4, 0x87, 0x71),
                        LineKind::Info => Color32::from_rgb(0x56, 0x9c, 0xd6),
                        LineKind::Echo => Color32::from_rgb(0xdc, 0xdc, 0xaa),
                    };
                    job.append(
                        text,
                        0.0,
                        TextFormat {
                            font_id: font.clone(),
                            color,
                            ..Default::default()
                        },
                    );
                }
                ui.add(egui::Label::new(job).selectable(true).wrap());
            });

        let busy = self.runner.is_busy();
        let (prompt, hint) = if busy && self.running_program {
            ("›", texts.input_hint)
        } else {
            ("earth›", texts.repl_hint)
        };
        ui.horizontal(|ui| {
            ui.label(
                RichText::new(prompt)
                    .monospace()
                    .color(Color32::from_rgb(0x89, 0xd1, 0x85)),
            );
            let response = ui.add_enabled(
                !busy || self.running_program,
                TextEdit::singleline(&mut self.terminal_input)
                    .font(font)
                    .hint_text(hint)
                    .desired_width(f32::INFINITY),
            );
            if self.focus_terminal {
                response.request_focus();
                self.focus_terminal = false;
            }
            if response.lost_focus() && ui.input(|i| i.key_pressed(Key::Enter)) {
                self.submit_terminal_line();
            }
        });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        ui.horizontal(|ui| {
            let state = if self.waiting_for_input {
                texts.waiting_for_input
            } else if self.runner.is_busy() {
                texts.running
            } else {
                texts.ready
            };
            ui.label(RichText::new(state).small());
            if let Some(status) = &self.status {
                ui.separator();
                ui.label(RichText::new(status).small());
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let document = &self.documents[self.active];
                let program_lang = match earth::detect_language(&document.text) {
                    Lang::Tr => "Earth (Türkçe)",
                    Lang::En => "Earth (English)",
                };
                ui.label(RichText::new("UTF-8").small());
                ui.separator();
                ui.label(RichText::new(program_lang).small());
                ui.separator();
                let position = texts
                    .line_column
                    .replace("{line}", &self.cursor.0.to_string())
                    .replace("{column}", &self.cursor.1.to_string());
                ui.label(RichText::new(position).small());
            });
        });
    }

    fn dialogs(&mut self, context: &egui::Context) {
        let Some(confirm) = &self.confirm else {
            return;
        };
        let texts = self.texts();
        let (title, body) = match confirm {
            Confirm::Quit => (texts.unsaved_title, texts.unsaved_body),
            Confirm::CloseTab(_) => (texts.unsaved_title, texts.close_tab_unsaved),
        };
        let mut choice = None;
        egui::Window::new(title)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(context, |ui| {
                ui.label(body);
                ui.add_space(8.0);
                ui.horizontal(|ui| match confirm {
                    Confirm::Quit => {
                        if ui.button(texts.save_all_and_quit).clicked() {
                            choice = Some(0);
                        }
                        if ui.button(texts.quit_without_saving).clicked() {
                            choice = Some(1);
                        }
                        if ui.button(texts.cancel).clicked() {
                            choice = Some(2);
                        }
                    }
                    Confirm::CloseTab(_) => {
                        if ui.button(texts.save).clicked() {
                            choice = Some(0);
                        }
                        if ui.button(texts.close_without_saving).clicked() {
                            choice = Some(1);
                        }
                        if ui.button(texts.cancel).clicked() {
                            choice = Some(2);
                        }
                    }
                });
            });

        let Some(choice) = choice else {
            return;
        };
        let confirm = self.confirm.take();
        match (confirm, choice) {
            (Some(Confirm::Quit), 0) => {
                let all_saved = (0..self.documents.len())
                    .all(|index| !self.documents[index].is_dirty() || self.save(index));
                if all_saved {
                    self.allow_close = true;
                    context.send_viewport_cmd(ViewportCommand::Close);
                }
            }
            (Some(Confirm::Quit), 1) => {
                self.allow_close = true;
                context.send_viewport_cmd(ViewportCommand::Close);
            }
            (Some(Confirm::CloseTab(index)), 0) => {
                if self.save(index) {
                    self.close_tab(index);
                }
            }
            (Some(Confirm::CloseTab(index)), 1) => self.close_tab(index),
            _ => {}
        }
    }

    fn shortcuts(&mut self, context: &egui::Context) {
        if context.input_mut(|i| i.consume_shortcut(&STOP)) {
            self.runner.stop();
        }
        if context.input_mut(|i| i.consume_shortcut(&RUN)) {
            self.run();
        }
        if context.input_mut(|i| i.consume_shortcut(&SAVE)) {
            self.save(self.active);
        }
        if context.input_mut(|i| i.consume_shortcut(&NEW)) {
            self.new_document();
        }
    }
}

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.handle_events();
        self.shortcuts(&context);

        if context.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.documents.iter().any(Document::is_dirty)
        {
            context.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Quit);
        }

        egui::Panel::top("toolbar").show(ui, |ui| {
            ui.add_space(4.0);
            self.toolbar(ui);
            ui.add_space(2.0);
        });
        egui::Panel::bottom("status").show(ui, |ui| self.status_bar(ui));
        egui::Panel::bottom("terminal")
            .resizable(true)
            .default_size(220.0)
            .min_size(120.0)
            .show(ui, |ui| self.terminal(ui));
        egui::Panel::left("explorer")
            .resizable(true)
            .default_size(230.0)
            .min_size(150.0)
            .show(ui, |ui| self.explorer(ui));
        egui::CentralPanel::default().show(ui, |ui| {
            self.tabs(ui);
            ui.separator();
            self.editor(ui);
        });

        self.dialogs(&context);
    }
}

/// The Earth logo for the toolbar, decoded with the same PNG reader that
/// provides the window icon.
fn load_logo(context: &egui::Context) -> Option<egui::TextureHandle> {
    let icon = eframe::icon_data::from_png_bytes(include_bytes!("../../windows/earth.png")).ok()?;
    let image = egui::ColorImage::from_rgba_unmultiplied(
        [icon.width as usize, icon.height as usize],
        &icon.rgba,
    );
    Some(context.load_texture("earth-logo", image, egui::TextureOptions::LINEAR))
}

fn show_nodes(
    ui: &mut egui::Ui,
    nodes: &[Node],
    active: Option<&Path>,
    clicked: &mut Option<PathBuf>,
) {
    for node in nodes {
        match &node.children {
            Some(children) => {
                egui::CollapsingHeader::new(format!("📁 {}", node.name))
                    .id_salt(&node.path)
                    .show(ui, |ui| show_nodes(ui, children, active, clicked));
            }
            None => {
                let icon = if node.name.ends_with(".earth") {
                    "🌍"
                } else {
                    "📄"
                };
                let selected = active == Some(node.path.as_path());
                if ui
                    .selectable_label(selected, format!("{icon} {}", node.name))
                    .clicked()
                {
                    *clicked = Some(node.path.clone());
                }
            }
        }
    }
}

fn line_numbers(count: usize, error_line: Option<usize>, font: &FontId) -> LayoutJob {
    let width = count.to_string().len().max(3);
    let mut job = LayoutJob::default();
    for line in 1..=count {
        let is_error = error_line == Some(line);
        let text = if line == count {
            format!("{line:>width$} ")
        } else {
            format!("{line:>width$} \n")
        };
        job.append(
            &text,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color: if is_error {
                    Color32::from_rgb(0xf4, 0x87, 0x71)
                } else {
                    Color32::from_gray(0x6e)
                },
                ..Default::default()
            },
        );
    }
    job
}

/// 1-based line and column of a character index.
fn line_and_column(text: &str, index: usize) -> (usize, usize) {
    let mut line = 1;
    let mut column = 1;
    for c in text.chars().take(index) {
        if c == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }
    (line, column)
}

fn configure_style(context: &egui::Context) {
    // The syntax colours are made for a dark background, so Studio stays
    // dark even when the system theme is light.
    context.set_theme(egui::Theme::Dark);
    context.all_styles_mut(|style| {
        style.spacing.item_spacing = egui::vec2(8.0, 6.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        for (text_style, font) in style.text_styles.iter_mut() {
            font.size = match text_style {
                egui::TextStyle::Small => 12.0,
                egui::TextStyle::Heading => 20.0,
                _ => 14.5,
            };
        }
    });
}

#[cfg(test)]
mod tests {
    use super::line_and_column;

    #[test]
    fn finds_line_and_column() {
        assert_eq!(line_and_column("ab\nçd", 0), (1, 1));
        assert_eq!(line_and_column("ab\nçd", 4), (2, 2));
    }
}
