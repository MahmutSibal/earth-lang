//! The Earth Studio window: explorer, tabbed editor, terminal and the
//! assistant, each in its own panel.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use earth::Lang;
use eframe::egui::{
    self, text::LayoutJob, Align, Color32, FontId, Key, Layout, RichText, ScrollArea, TextEdit,
    TextFormat, ViewportCommand,
};

use crate::assistant::{ChatError, Entry, Job};
use crate::auth::{self, Failure, Profile, Provider, SignIn};
use crate::files::{self, Node};
use crate::runner::{Event, Resume, Runner};
use crate::settings::{Settings, Side};
use crate::shortcuts::{Action, Bindings};
use crate::texts::{texts, Texts};
use crate::theme::{self, Palette, ThemeName};
use crate::update::{self, Status, Updater};
use crate::workspace;

/// Older terminal output is dropped beyond this many characters.
const TERMINAL_LIMIT: usize = 200_000;

mod assistant_panel;
mod debug_panel;
mod editor;
mod settings_window;

struct Document {
    path: Option<PathBuf>,
    text: String,
    saved_text: String,
    /// The line of the last error reported in this file, 1-based.
    error_line: Option<usize>,
    /// Lines to pause on when debugging.
    breakpoints: BTreeSet<usize>,
    /// The first syntax error, found while typing.
    syntax_error: Option<editor::SyntaxError>,
    /// The text the syntax check last saw.
    checked_text: Option<String>,
}

impl Document {
    fn new(path: Option<PathBuf>, text: String) -> Self {
        Self {
            path,
            saved_text: text.clone(),
            text,
            error_line: None,
            breakpoints: BTreeSet::new(),
            syntax_error: None,
            checked_text: None,
        }
    }

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

enum AccountAction {
    SignIn(Provider),
    Cancel,
    Finish(Profile),
    SignOut,
}

enum Confirm {
    Quit,
    CloseTab(usize),
}

pub struct StudioApp {
    settings: Settings,
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
    updater: Updater,
    /// The user dismissed the "update ready" banner for this session.
    update_banner_hidden: bool,
    /// Install the update and reopen Studio once the window has closed.
    relaunch_for_update: bool,
    show_settings: bool,
    show_account: bool,
    sign_in: Option<SignIn>,
    /// The sign-in page has been opened in the browser.
    browser_opened: bool,
    /// A downloaded profile picture, waiting to become `avatar`.
    avatar_image: Arc<Mutex<Option<egui::ColorImage>>>,
    avatar: Option<egui::TextureHandle>,
    wake: Arc<dyn Fn() + Send + Sync>,
    // ----- assistant -----
    /// The API key of the assistant's service, if it needs one.
    api_key: Option<String>,
    /// The service set-up form, while it is open.
    setup: Option<assistant_panel::Setup>,
    chat: Vec<Entry>,
    chat_input: String,
    chat_job: Option<Job>,
    chat_error: Option<ChatError>,
    send_open_file: bool,
    /// New text for the open file, waiting for the user to approve it.
    replace_preview: Option<String>,
    /// The document and its text before the assistant last changed it.
    undo: Option<(usize, String)>,
    /// The last error of a program run, for "Explain error".
    last_error: Option<String>,
    /// The cursor as a character index, where "Insert" puts code.
    cursor_index: Option<usize>,
    // ----- editor and debugger -----
    completion: Option<editor::Completion>,
    /// Scroll the editor to this line on the next frame.
    scroll_to_line: Option<usize>,
    debug: Option<debug_panel::DebugSession>,
    bindings: Bindings,
    /// The theme and scale last applied, to apply changes once.
    applied_look: Option<(String, f32)>,
    settings_tab: settings_window::Tab,
    /// The action whose new shortcut is being typed in the settings.
    capturing: Option<Action>,
    /// The teacher's lesson being worked on, by number.
    current_lesson: Option<usize>,
}

impl StudioApp {
    pub fn new(context: &eframe::CreationContext<'_>, start: Option<PathBuf>) -> Self {
        let egui_context = context.egui_ctx.clone();
        let update_context = context.egui_ctx.clone();
        let settings = Settings::load();
        let bindings = Bindings::from_settings(&settings.shortcuts);
        let lang = settings.lang;
        let auto_update = settings.auto_update;
        let wake_context = context.egui_ctx.clone();
        let mut app = Self {
            settings,
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
            updater: Updater::start(auto_update, move || update_context.request_repaint()),
            update_banner_hidden: false,
            relaunch_for_update: false,
            show_settings: false,
            show_account: false,
            sign_in: None,
            browser_opened: false,
            avatar_image: Arc::new(Mutex::new(None)),
            avatar: None,
            wake: Arc::new(move || wake_context.request_repaint()),
            api_key: None,
            setup: None,
            chat: Vec::new(),
            chat_input: String::new(),
            chat_job: None,
            chat_error: None,
            send_open_file: true,
            replace_preview: None,
            undo: None,
            last_error: None,
            cursor_index: None,
            completion: None,
            scroll_to_line: None,
            debug: None,
            bindings,
            applied_look: None,
            settings_tab: settings_window::Tab::General,
            capturing: None,
            current_lesson: None,
        };
        app.load_avatar();
        crate::credentials::forget_github_token();
        app.api_key = crate::credentials::load_api_key(&app.settings.ai_service);

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
            // Started from the Start menu or desktop: open the user's Earth
            // folder, created with the examples on first run.
            None => {
                if let Some(folder) = workspace::prepare_earth_folder() {
                    app.open_folder(folder);
                }
            }
        }
        if app.documents.is_empty() {
            let welcome = texts(lang).welcome.to_owned();
            app.documents.push(Document::new(None, welcome));
        }
        app
    }

    fn texts(&self) -> &'static Texts {
        texts(self.settings.lang)
    }

    fn palette(&self) -> &'static Palette {
        theme::palette(ThemeName::from_id(&self.settings.theme).unwrap_or(ThemeName::Midnight))
    }

    /// Applies the theme and scale when they changed.
    fn apply_look(&mut self, context: &egui::Context) {
        let look = (self.settings.theme.clone(), self.settings.ui_scale);
        if self.applied_look.as_ref() != Some(&look) {
            theme::apply(context, self.palette());
            context.set_zoom_factor(self.settings.ui_scale);
            self.applied_look = Some(look);
        }
    }

    // ----- account -----

    /// Downloads the signed-in user's picture in the background.
    fn load_avatar(&mut self) {
        self.avatar = None;
        let Some(url) = self.settings.profile.as_ref().map(|p| p.avatar_url.clone()) else {
            return;
        };
        let target = Arc::clone(&self.avatar_image);
        let wake = Arc::clone(&self.wake);
        let _ = std::thread::Builder::new()
            .name("earth-avatar".to_owned())
            .spawn(move || {
                if let Some(image) = auth::fetch_avatar(&url) {
                    if let Ok(mut slot) = target.lock() {
                        *slot = Some(image);
                    }
                    wake();
                }
            });
    }

    fn start_sign_in(&mut self, context: &egui::Context, provider: Provider) {
        self.sign_in = Some(SignIn::start(provider, Arc::clone(&self.wake)));
        self.browser_opened = false;
        // The browser opens once the sign-in knows its address; see
        // `account_window`.
        context.request_repaint();
    }

    fn finish_sign_in(&mut self, profile: Profile) {
        self.settings.profile = Some(profile);
        self.settings.save();
        self.sign_in = None;
        self.load_avatar();
    }

    fn sign_out(&mut self) {
        self.settings.profile = None;
        self.settings.save();
        self.avatar = None;
        if let Ok(mut slot) = self.avatar_image.lock() {
            *slot = None;
        }
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
                self.documents
                    .push(Document::new(Some(path.to_path_buf()), text));
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

    /// Writes the built-in examples into the Earth folder and opens it.
    fn show_examples(&mut self) {
        let Some(folder) = workspace::earth_folder() else {
            return;
        };
        match workspace::write_examples(&folder) {
            Ok(examples) => {
                self.open_folder(folder);
                self.open_file(&examples.join(workspace::EXAMPLES[0].0));
            }
            Err(error) => {
                self.status = Some(format!("{}: {error}", self.texts().could_not_save));
            }
        }
    }

    fn new_document(&mut self) {
        self.documents.push(Document::new(None, String::new()));
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
        self.debug = None;
        self.start_program(false);
    }

    fn start_program(&mut self, debug: bool) {
        self.completion = None;
        self.save_all_named();
        self.last_error = None;
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
        self.runner.run(source, base_dir, file, debug);
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
            self.runner.repl(line, self.settings.lang);
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
                Event::Paused(snapshot) => self.paused_at(snapshot),
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
                    self.debug = None;
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
        if self.running_program {
            self.last_error = Some(format!("{place}{text}"));
        }
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
        let palette = self.palette();
        let hint = |app: &Self, text: &str, action: Action| {
            format!(
                "{text} ({})",
                crate::shortcuts::format(&app.bindings.get(action))
            )
        };
        ui.horizontal(|ui| {
            if let Some(logo) = &self.logo {
                ui.add(egui::Image::new(logo).fit_to_exact_size(egui::vec2(26.0, 26.0)));
            }
            ui.label(RichText::new("Earth Studio").strong().size(16.0));
            ui.add_space(12.0);
            if icon_button(ui, "📂", texts.open_folder_hint).clicked() {
                if let Some(folder) = rfd::FileDialog::new().pick_folder() {
                    self.open_folder(folder);
                }
            }
            if icon_button(ui, "📚", texts.examples_hint).clicked() {
                self.show_examples();
            }
            if icon_button(
                ui,
                "📄",
                &hint(self, texts.action_new_file, Action::NewFile),
            )
            .clicked()
            {
                self.new_document();
            }
            if icon_button(ui, "💾", &hint(self, texts.action_save, Action::Save)).clicked() {
                self.save(self.active);
            }
            ui.add_space(8.0);
            let busy = self.runner.is_busy();
            let run = egui::Button::new(RichText::new(texts.run).color(Color32::WHITE).strong())
                .fill(palette.accent)
                .min_size(egui::vec2(0.0, 28.0));
            if ui
                .add_enabled(!busy, run)
                .on_hover_text(hint(self, texts.action_run, Action::Run))
                .clicked()
            {
                self.run();
            }
            let debug = egui::Button::new(texts.debug_button).min_size(egui::vec2(0.0, 28.0));
            if ui
                .add_enabled(!busy, debug)
                .on_hover_text(hint(self, texts.action_debug, Action::Debug))
                .clicked()
            {
                self.start_debugging();
            }
            let stop = egui::Button::new(RichText::new("⏹").color(palette.danger).size(16.0))
                .min_size(egui::vec2(32.0, 28.0));
            if ui
                .add_enabled(busy, stop)
                .on_hover_text(hint(self, texts.action_stop, Action::Stop))
                .clicked()
            {
                self.runner.stop();
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let current = match self.settings.lang {
                    Lang::Tr => "Türkçe",
                    Lang::En => "English",
                };
                egui::ComboBox::from_id_salt("language")
                    .selected_text(format!("🌐 {current}"))
                    .width(120.0)
                    .show_ui(ui, |ui| {
                        let before = self.settings.lang;
                        ui.selectable_value(&mut self.settings.lang, Lang::Tr, "Türkçe");
                        ui.selectable_value(&mut self.settings.lang, Lang::En, "English");
                        if self.settings.lang != before {
                            self.settings.save();
                        }
                    });
                let assistant = ui
                    .selectable_label(
                        self.settings.assistant_open,
                        RichText::new(texts.assistant).size(15.0),
                    )
                    .on_hover_text(hint(self, texts.action_assistant, Action::Assistant));
                if assistant.clicked() {
                    self.toggle_assistant();
                }
            });
        });
    }

    fn explorer(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        ui.horizontal(|ui| {
            ui.label(RichText::new(texts.explorer_title).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if self.folder.is_some() {
                    if small_icon(ui, "🔄", texts.refresh_hint).clicked() {
                        self.refresh_tree();
                    }
                    if small_icon(ui, "➕", texts.new_file_in_folder_hint).clicked() {
                        self.new_file_name = Some(String::new());
                    }
                }
            });
        });
        ui.add_space(4.0);

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
            ui.label(RichText::new(texts.no_folder).weak());
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

        let active_path = self.documents[self.active].path.clone();
        let mut clicked = None;
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                egui::CollapsingHeader::new(RichText::new(format!("📂 {folder_name}")).strong())
                    .id_salt("root")
                    .default_open(true)
                    .show(ui, |ui| {
                        show_nodes(ui, &self.tree, active_path.as_deref(), &mut clicked);
                    });
            });
        if let Some(path) = clicked {
            self.open_file(&path);
        }
    }

    fn tabs(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let palette = self.palette();
        let mut close = None;
        ScrollArea::horizontal().id_salt("tabs").show(ui, |ui| {
            ui.horizontal(|ui| {
                for (index, document) in self.documents.iter().enumerate() {
                    let selected = index == self.active;
                    let fill = if selected {
                        palette.selection
                    } else {
                        Color32::TRANSPARENT
                    };
                    egui::Frame::new()
                        .fill(fill)
                        .corner_radius(egui::CornerRadius::same(6))
                        .inner_margin(egui::Margin::symmetric(8, 3))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                let icon = if document.syntax_error.is_some() {
                                    "⚠"
                                } else {
                                    "📄"
                                };
                                let title =
                                    RichText::new(format!("{icon} {}", document.title(texts)));
                                let title = if selected {
                                    title.strong()
                                } else {
                                    title.color(palette.weak_text)
                                };
                                let label = egui::Label::new(title).sense(egui::Sense::click());
                                if ui.add(label).clicked() {
                                    self.active = index;
                                }
                                if ui
                                    .add(
                                        egui::Button::new(
                                            RichText::new("×").color(palette.weak_text),
                                        )
                                        .frame(false),
                                    )
                                    .clicked()
                                {
                                    close = Some(index);
                                }
                            });
                        });
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

    fn terminal(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let palette = self.palette();
        // Fill the panel, or the resizable panel shrinks to its content.
        ui.set_min_height(ui.available_height());
        ui.horizontal(|ui| {
            ui.label(RichText::new(format!("🖥 {}", texts.terminal_title)).strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.small_button(texts.clear).clicked() {
                    self.terminal.clear();
                }
                if let Some(error) = self.last_error.clone() {
                    if ui.small_button(texts.assistant_explain_error).clicked() {
                        self.explain_error(&error);
                    }
                }
            });
        });
        ui.add_space(2.0);

        let font = FontId::monospace(self.settings.font_size - 1.0);
        let input_height = 36.0;
        egui::Frame::new()
            .fill(palette.editor)
            .corner_radius(egui::CornerRadius::same(6))
            .inner_margin(egui::Margin::same(8))
            .show(ui, |ui| {
                ScrollArea::vertical()
                    .id_salt("terminal")
                    .stick_to_bottom(true)
                    .auto_shrink([false, false])
                    .max_height(ui.available_height() - input_height)
                    .show(ui, |ui| {
                        let mut job = LayoutJob::default();
                        for (text, kind) in &self.terminal {
                            let color = match kind {
                                LineKind::Output => palette.text,
                                LineKind::Error => palette.danger,
                                LineKind::Info => palette.accent,
                                LineKind::Echo => palette.warning,
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
                    ("earth>", texts.repl_hint)
                };
                ui.horizontal(|ui| {
                    ui.label(RichText::new(prompt).monospace().color(palette.success));
                    let response = ui.add_enabled(
                        !busy || self.running_program,
                        TextEdit::singleline(&mut self.terminal_input)
                            .font(font)
                            .hint_text(hint)
                            .frame(egui::Frame::NONE)
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
            });
    }

    fn status_bar(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        let palette = self.palette();
        ui.horizontal(|ui| {
            if let Some(downloads) = self.updater.downloads() {
                let separator = if self.settings.lang == Lang::Tr {
                    '.'
                } else {
                    ','
                };
                let text = texts
                    .downloads
                    .replace("{count}", &update::group_digits(downloads, separator));
                ui.label(RichText::new(text).small().color(palette.weak_text))
                    .on_hover_text(texts.downloads_hint);
                ui.add_space(10.0);
            }
            let (state, color) = if self.is_paused() {
                (texts.debug_paused, palette.warning)
            } else if self.waiting_for_input {
                (texts.waiting_for_input, palette.warning)
            } else if self.runner.is_busy() {
                (texts.running, palette.accent)
            } else {
                (texts.ready, palette.success)
            };
            let (dot, _) = ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
            ui.painter().circle_filled(dot.center(), 4.0, color);
            ui.label(RichText::new(state).small());
            let update_state = match self.updater.status() {
                Status::Downloading { version, percent } => Some(
                    texts
                        .update_downloading
                        .replace("{version}", &version)
                        .replace("{percent}", &percent.to_string()),
                ),
                Status::UpToDate => Some(texts.up_to_date.to_owned()),
                _ => None,
            };
            if let Some(update_state) = update_state {
                ui.separator();
                ui.label(RichText::new(update_state).small().color(palette.weak_text));
            }
            if let Some(status) = &self.status {
                ui.separator();
                ui.label(RichText::new(status).small().color(palette.weak_text));
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let document = &self.documents[self.active];
                let program_lang = match earth::detect_language(&document.text) {
                    Lang::Tr => "Earth (Türkçe)",
                    Lang::En => "Earth (English)",
                };
                let version = ui
                    .add(
                        egui::Label::new(
                            RichText::new(concat!("v", env!("CARGO_PKG_VERSION"))).small(),
                        )
                        .sense(egui::Sense::click()),
                    )
                    .on_hover_text(texts.check_updates);
                if version.clicked() {
                    self.updater.check_now();
                }
                ui.separator();
                ui.label(RichText::new(program_lang).small());
                ui.separator();
                let position = texts
                    .line_column
                    .replace("{line}", &self.cursor.0.to_string())
                    .replace("{column}", &self.cursor.1.to_string());
                ui.label(RichText::new(position).small());
                if let Some(error) = &document.syntax_error {
                    ui.separator();
                    let line = error.line;
                    let column = error.column;
                    let response = ui
                        .add(
                            egui::Label::new(
                                RichText::new(texts.syntax_errors_count)
                                    .small()
                                    .color(palette.danger),
                            )
                            .sense(egui::Sense::click()),
                        )
                        .on_hover_text(&error.message);
                    if response.clicked() {
                        self.go_to(ui.ctx(), line, column);
                    }
                }
            });
        });
    }

    /// The strip under the toolbar that announces a new version.
    fn update_banner(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        ui.horizontal(|ui| match self.updater.status() {
            Status::Ready { version } => {
                ui.label(
                    RichText::new(texts.update_ready.replace("{version}", &version))
                        .color(Color32::from_rgb(0x89, 0xd1, 0x85)),
                );
                if ui.button(texts.update_now).clicked() {
                    self.relaunch_for_update = true;
                    ui.ctx().send_viewport_cmd(ViewportCommand::Close);
                }
                if ui.button(texts.update_later).clicked() {
                    self.update_banner_hidden = true;
                }
            }
            Status::Available { version } => {
                ui.label(texts.update_available.replace("{version}", &version));
                if ui.button(texts.update_download).clicked() {
                    ui.ctx().open_url(egui::OpenUrl::new_tab(format!(
                        "{}/releases/latest",
                        update::PROJECT_URL
                    )));
                }
                if ui.button(texts.update_later).clicked() {
                    self.update_banner_hidden = true;
                }
            }
            _ => {}
        });
    }

    /// The bottom-left corner: the signed-in user (or "Sign in") and the
    /// settings button.
    fn account_bar(&mut self, ui: &mut egui::Ui) {
        let texts = self.texts();
        if let Some(image) = self
            .avatar_image
            .lock()
            .ok()
            .and_then(|mut slot| slot.take())
        {
            self.avatar = Some(ui.ctx().load_texture(
                "avatar",
                image,
                egui::TextureOptions::LINEAR,
            ));
        }
        ui.horizontal(|ui| {
            let account = match &self.settings.profile {
                Some(profile) => {
                    self.avatar_or_initial(ui, 22.0);
                    ui.add(
                        egui::Button::new(RichText::new(&profile.name).strong())
                            .frame(false)
                            .truncate(),
                    )
                }
                None => ui.add(egui::Button::new(texts.account_sign_in).frame(false)),
            };
            if account.on_hover_text(texts.account_hint).clicked() {
                self.show_account = !self.show_account;
                self.show_settings = false;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let settings = ui
                    .add(egui::Button::new(RichText::new("⚙").size(18.0)).frame(false))
                    .on_hover_text(texts.settings_hint);
                if settings.clicked() {
                    self.show_settings = !self.show_settings;
                    self.show_account = false;
                }
            });
        });
    }

    fn avatar_or_initial(&self, ui: &mut egui::Ui, size: f32) {
        match &self.avatar {
            Some(avatar) => {
                ui.add(
                    egui::Image::new(avatar)
                        .fit_to_exact_size(egui::vec2(size, size))
                        .corner_radius(size / 2.0),
                );
            }
            None => {
                let initial = self
                    .settings
                    .profile
                    .as_ref()
                    .and_then(|profile| profile.name.chars().next())
                    .map(|c| c.to_uppercase().to_string())
                    .unwrap_or_else(|| "?".to_owned());
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(size, size), egui::Sense::hover());
                ui.painter().circle_filled(
                    rect.center(),
                    size / 2.0,
                    Color32::from_rgb(0x2b, 0x6c, 0xb0),
                );
                ui.painter().text(
                    rect.center(),
                    egui::Align2::CENTER_CENTER,
                    initial,
                    FontId::proportional(size * 0.5),
                    Color32::WHITE,
                );
            }
        }
    }

    fn account_window(&mut self, context: &egui::Context) {
        if !self.show_account {
            return;
        }
        let texts = self.texts();
        let mut open = true;
        let mut action = None;
        egui::Window::new(texts.account_title)
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .default_width(320.0)
            .anchor(egui::Align2::LEFT_BOTTOM, [8.0, -66.0])
            .show(context, |ui| {
                ui.set_max_width(320.0);
                if let Some(profile) = &self.settings.profile {
                    ui.horizontal(|ui| {
                        self.avatar_or_initial(ui, 56.0);
                        ui.vertical(|ui| {
                            ui.label(RichText::new(&profile.name).strong().size(17.0));
                            ui.label(&profile.account);
                            ui.label(
                                RichText::new(match profile.provider {
                                    Provider::Google => texts.google_account,
                                    Provider::GitHub => texts.github_account,
                                })
                                .small()
                                .weak(),
                            );
                        });
                    });
                    ui.add_space(8.0);
                    if ui.button(texts.sign_out).clicked() {
                        action = Some(AccountAction::SignOut);
                    }
                    return;
                }

                match self.sign_in.as_ref().map(|sign_in| sign_in.state()) {
                    None => {
                        ui.label(texts.account_intro);
                        ui.add_space(8.0);
                        let google = ui.add_enabled(
                            SignIn::google_available(),
                            egui::Button::new(texts.sign_in_google)
                                .min_size(egui::vec2(220.0, 30.0)),
                        );
                        if google.clicked() {
                            action = Some(AccountAction::SignIn(Provider::Google));
                        }
                        google.on_disabled_hover_text(texts.google_unavailable);
                        let github = ui.add(
                            egui::Button::new(texts.sign_in_github)
                                .min_size(egui::vec2(220.0, 30.0)),
                        );
                        if github.clicked() {
                            action = Some(AccountAction::SignIn(Provider::GitHub));
                        }
                    }
                    Some(auth::State::Starting) => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(texts.waiting);
                        });
                    }
                    Some(auth::State::InBrowser { url }) => {
                        if !self.browser_opened {
                            self.browser_opened = true;
                            ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                        }
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(texts.in_browser);
                        });
                        ui.horizontal(|ui| {
                            if ui.button(texts.open_browser_again).clicked() {
                                ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                            }
                            if ui.button(texts.cancel).clicked() {
                                action = Some(AccountAction::Cancel);
                            }
                        });
                    }
                    Some(auth::State::DeviceCode { user_code, url }) => {
                        if !self.browser_opened {
                            self.browser_opened = true;
                            ui.ctx().copy_text(user_code.clone());
                            ui.ctx().open_url(egui::OpenUrl::new_tab(&url));
                        }
                        ui.label(texts.device_code_intro);
                        ui.label(
                            RichText::new(&user_code)
                                .monospace()
                                .size(26.0)
                                .strong()
                                .color(Color32::from_rgb(0x89, 0xd1, 0x85)),
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
                                action = Some(AccountAction::Cancel);
                            }
                        });
                    }
                    Some(auth::State::Done(profile)) => {
                        action = Some(AccountAction::Finish(profile));
                    }
                    Some(auth::State::Failed(failure)) => {
                        let message = match failure {
                            Failure::Denied => texts.failure_denied,
                            Failure::TimedOut => texts.failure_timed_out,
                            Failure::Network => texts.failure_network,
                            Failure::DeviceFlowDisabled => texts.failure_device_flow,
                        };
                        ui.label(RichText::new(message).color(Color32::from_rgb(0xf4, 0x87, 0x71)));
                        ui.add_space(6.0);
                        if ui.button(texts.cancel).clicked() {
                            action = Some(AccountAction::Cancel);
                        }
                    }
                }
            });
        match action {
            Some(AccountAction::SignIn(provider)) => self.start_sign_in(context, provider),
            Some(AccountAction::Cancel) => self.sign_in = None,
            Some(AccountAction::Finish(profile)) => self.finish_sign_in(profile),
            Some(AccountAction::SignOut) => self.sign_out(),
            None => {}
        }
        if !open {
            self.show_account = false;
            self.sign_in = None;
        }
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
        if self.capturing.is_some() {
            return;
        }
        let pressed: Vec<Action> = Action::ALL
            .into_iter()
            .filter(|action| *action != Action::Complete)
            .filter(|action| {
                let shortcut = self.bindings.get(*action);
                context.input_mut(|input| input.consume_shortcut(&shortcut))
            })
            .collect();
        let paused = self.is_paused();
        for action in pressed {
            match action {
                Action::Stop => self.runner.stop(),
                // Like VS Code, Run goes on when paused.
                Action::Run if paused => self.resume(Resume::Continue),
                Action::Run => self.run(),
                Action::Debug => self.start_debugging(),
                Action::Continue => self.resume(Resume::Continue),
                Action::StepOver => self.resume(Resume::StepOver),
                Action::StepInto => self.resume(Resume::StepInto),
                Action::StepOut => self.resume(Resume::StepOut),
                Action::ToggleBreakpoint => self.toggle_breakpoint(self.cursor.0),
                Action::Save => {
                    self.save(self.active);
                }
                Action::NewFile => self.new_document(),
                Action::Assistant => self.toggle_assistant(),
                Action::Complete => {}
            }
        }
    }
}

/// A side panel on the side the settings ask for.
fn side_panel(side: Side, id: &'static str) -> egui::Panel {
    match side {
        Side::Left => egui::Panel::left(id),
        Side::Right => egui::Panel::right(id),
    }
}

fn icon_button(ui: &mut egui::Ui, icon: &str, hint: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(icon).size(16.0)).min_size(egui::vec2(32.0, 28.0)))
        .on_hover_text(hint)
}

fn small_icon(ui: &mut egui::Ui, icon: &str, hint: &str) -> egui::Response {
    ui.add(egui::Button::new(RichText::new(icon).size(13.0)).frame(false))
        .on_hover_text(hint)
}

impl eframe::App for StudioApp {
    /// A downloaded update is installed as Studio closes: right away with
    /// a progress window and a restart when the user asked for it, silently
    /// otherwise, so the next start already runs the new version.
    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        if matches!(self.updater.status(), Status::Ready { .. }) {
            self.updater.install(self.relaunch_for_update);
        }
    }

    /// Behind the panels: the theme's window colour.
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        self.palette().window.to_normalized_gamma_f32()
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let context = ui.ctx().clone();
        self.apply_look(&context);
        self.handle_events();
        self.shortcuts(&context);
        self.live_check();

        if context.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && self.documents.iter().any(Document::is_dirty)
        {
            context.send_viewport_cmd(ViewportCommand::CancelClose);
            self.confirm = Some(Confirm::Quit);
        }

        let palette = self.palette();
        egui::Panel::top("toolbar")
            .frame(theme::card(palette).inner_margin(egui::Margin::symmetric(10, 6)))
            .show(ui, |ui| self.toolbar(ui));
        let announces_update = matches!(
            self.updater.status(),
            Status::Ready { .. } | Status::Available { .. }
        );
        if announces_update && !self.update_banner_hidden {
            egui::Panel::top("update")
                .frame(
                    theme::card(palette)
                        .fill(palette.success.gamma_multiply(0.15))
                        .inner_margin(egui::Margin::symmetric(10, 6)),
                )
                .show(ui, |ui| self.update_banner(ui));
        }
        egui::Panel::bottom("status")
            .frame(
                egui::Frame::new()
                    .fill(palette.window)
                    .inner_margin(egui::Margin::symmetric(12, 4)),
            )
            .show(ui, |ui| self.status_bar(ui));
        side_panel(self.settings.explorer_side, "explorer")
            .resizable(true)
            .default_size(240.0)
            .min_size(160.0)
            .frame(theme::card(palette))
            .show(ui, |ui| {
                egui::Panel::bottom("account")
                    .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(0, 6)))
                    .show(ui, |ui| self.account_bar(ui));
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| self.explorer(ui));
            });
        if self.settings.assistant_open {
            side_panel(self.settings.assistant_side, "assistant")
                .resizable(true)
                .default_size(380.0)
                .min_size(280.0)
                .frame(theme::card(palette))
                .show(ui, |ui| self.assistant_panel(ui));
        }
        // After the side panels, so that they run the full height and the
        // terminal sits under the editor, as in VS Code.
        egui::Panel::bottom("terminal")
            .resizable(true)
            .default_size(290.0)
            .min_size(120.0)
            .frame(theme::card(palette))
            .show(ui, |ui| {
                if self.debug.is_some() {
                    egui::Panel::right("debug")
                        .resizable(true)
                        .default_size(400.0)
                        .min_size(260.0)
                        .frame(egui::Frame::new().inner_margin(egui::Margin {
                            left: 10,
                            ..Default::default()
                        }))
                        .show(ui, |ui| self.debug_panel(ui));
                }
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ui, |ui| self.terminal(ui));
            });
        egui::CentralPanel::default()
            .frame(theme::card(palette))
            .show(ui, |ui| {
                self.tabs(ui);
                ui.add_space(4.0);
                if self.documents[self.active].syntax_error.is_some() {
                    egui::Panel::bottom("syntax")
                        .frame(egui::Frame::new().inner_margin(egui::Margin::symmetric(4, 4)))
                        .show(ui, |ui| self.syntax_bar(ui));
                }
                egui::CentralPanel::default()
                    .frame(
                        egui::Frame::new()
                            .fill(palette.editor)
                            .corner_radius(egui::CornerRadius::same(6)),
                    )
                    .show(ui, |ui| self.editor(ui));
            });

        self.poll_assistant();
        self.replace_window(&context);
        self.account_window(&context);
        self.settings_window(&context);
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

#[cfg(test)]
mod tests {
    use super::line_and_column;

    #[test]
    fn finds_line_and_column() {
        assert_eq!(line_and_column("ab\nçd", 0), (1, 1));
        assert_eq!(line_and_column("ab\nçd", 4), (2, 2));
    }
}
