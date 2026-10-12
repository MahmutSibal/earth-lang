//! The code editor: line numbers with breakpoints, syntax colours, the
//! syntax check that runs while typing, and suggestions as you type.

use eframe::egui::{
    self, text::CCursor, text::CCursorRange, Align, Align2, FontId, Key, Modifiers, RichText,
    ScrollArea, Sense, TextEdit,
};

use super::{line_and_column, StudioApp};
use crate::completion::{self, Kind, Suggestion};
use crate::highlight::{self, Marks};
use crate::shortcuts::Action;

/// How many suggestions the list shows.
const SUGGESTIONS: usize = 8;

/// The open suggestion list.
pub(super) struct Completion {
    /// Where the word being completed starts, as a character index.
    start: usize,
    items: Vec<Suggestion>,
    selected: usize,
    /// Accept the selected item on the next frame (it was clicked).
    accept: bool,
    /// Where the list was drawn, to keep it open when it is clicked.
    rect: Option<egui::Rect>,
}

/// A syntax error found while typing.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct SyntaxError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

/// Parses `source` and returns its first syntax error, if any, in the
/// program's language.
pub(super) fn check(source: &str) -> Option<SyntaxError> {
    let error = earth::parse(source).err()?;
    let position = error.position?;
    Some(SyntaxError {
        line: position.line,
        column: position.column,
        message: error.text(earth::detect_language(source)),
    })
}

impl StudioApp {
    fn text_id(&self) -> egui::Id {
        egui::Id::new(("editor-text", self.active))
    }

    /// Re-checks the open file when its text changed since the last check.
    pub(super) fn live_check(&mut self) {
        let document = &mut self.documents[self.active];
        if document.checked_text.as_deref() != Some(document.text.as_str()) {
            document.syntax_error = check(&document.text);
            document.checked_text = Some(document.text.clone());
        }
    }

    /// Moves the editor's cursor to a line and column (both 1-based).
    pub(super) fn go_to(&mut self, context: &egui::Context, line: usize, column: usize) {
        let text = &self.documents[self.active].text;
        let mut index = 0;
        for (number, content) in text.split('\n').enumerate() {
            if number + 1 == line {
                index += column.saturating_sub(1).min(content.chars().count());
                break;
            }
            index += content.chars().count() + 1;
        }
        self.set_cursor(context, index);
        self.scroll_to_line = Some(line);
    }

    fn set_cursor(&mut self, context: &egui::Context, index: usize) {
        let id = self.text_id();
        let mut state = egui::text_edit::TextEditState::load(context, id).unwrap_or_default();
        state
            .cursor
            .set_char_range(Some(CCursorRange::one(CCursor::new(index))));
        state.store(context, id);
        context.memory_mut(|memory| memory.request_focus(id));
        self.cursor_index = Some(index);
    }

    pub(super) fn toggle_breakpoint(&mut self, line: usize) {
        let breakpoints = &mut self.documents[self.active].breakpoints;
        if !breakpoints.remove(&line) {
            breakpoints.insert(line);
        }
        if let Some(session) = &self.debug {
            if session.document == self.active {
                self.runner
                    .set_breakpoints(&self.documents[self.active].breakpoints);
            }
        }
    }

    /// The line the debugger is paused on in the open file.
    fn paused_line(&self) -> Option<usize> {
        let session = self.debug.as_ref()?;
        let snapshot = session.paused.as_ref()?;
        (session.document == self.active && snapshot.file.is_none()).then_some(snapshot.line)
    }

    pub(super) fn editor(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let font = FontId::monospace(self.settings.font_size);
        let (row_height, digit_width) =
            ui.fonts_mut(|fonts| (fonts.row_height(&font), fonts.glyph_width(&font, '0')));
        let text_id = self.text_id();
        let has_focus = ui.memory(|memory| memory.has_focus(text_id));
        let paused_line = self.paused_line();
        let cursor_line = self.cursor.0;

        // Keys for the suggestion list, taken before the editor sees them.
        let mut accept = None;
        if let Some(completion) = &mut self.completion {
            let count = completion.items.len();
            if has_focus {
                ui.input_mut(|input| {
                    if input.consume_key(Modifiers::NONE, Key::ArrowDown) {
                        completion.selected = (completion.selected + 1) % count;
                    }
                    if input.consume_key(Modifiers::NONE, Key::ArrowUp) {
                        completion.selected = (completion.selected + count - 1) % count;
                    }
                    if input.consume_key(Modifiers::NONE, Key::Tab)
                        || input.consume_key(Modifiers::NONE, Key::Enter)
                    {
                        completion.accept = true;
                    }
                });
                if ui.input_mut(|input| input.consume_key(Modifiers::NONE, Key::Escape)) {
                    self.completion = None;
                }
            }
        }
        if let Some(completion) = self.completion.take_if(|completion| completion.accept) {
            accept = Some((
                completion.start,
                completion.items[completion.selected].clone(),
            ));
        }
        let complete_shortcut = self.bindings.get(Action::Complete);
        let force_complete =
            has_focus && ui.input_mut(|input| input.consume_shortcut(&complete_shortcut));

        if let Some((start, suggestion)) = accept {
            let cursor = self.cursor_index.unwrap_or(start);
            let document = &mut self.documents[self.active];
            let (insertion, offset) = suggestion.insertion();
            let byte = |index: usize| {
                document
                    .text
                    .char_indices()
                    .nth(index)
                    .map_or(document.text.len(), |(byte, _)| byte)
            };
            let range = byte(start)..byte(cursor.max(start));
            document.text.replace_range(range, &insertion);
            self.set_cursor(ui.ctx(), start + offset);
        }

        let active = self.active;
        let document = &mut self.documents[active];
        let marks = Marks {
            error_line: document.error_line,
            paused_line,
            squiggle: document
                .syntax_error
                .as_ref()
                .map(|error| (error.line, error.column)),
        };
        let error_lines = [
            document.error_line,
            document.syntax_error.as_ref().map(|error| error.line),
        ];
        let syntax = &palette.syntax;
        let mut layouter = |ui: &egui::Ui, text: &dyn egui::TextBuffer, _wrap_width: f32| {
            // No wrapping: each source line stays one row, matching the gutter.
            let job = highlight::layout(text.as_str(), font.clone(), syntax, &marks);
            ui.fonts_mut(|fonts| fonts.layout_job(job))
        };
        let line_count = document.text.split('\n').count();
        let digits = line_count.to_string().len().max(2);
        let gutter_width = 24.0 + digits as f32 * digit_width + 12.0;

        let mut changed = false;
        let mut cursor = None;
        let mut clicked_line = None;
        let mut popup_at = None;
        let scroll_to = self.scroll_to_line.take();
        ScrollArea::both()
            .id_salt(("editor", active))
            .auto_shrink([false, false])
            .show(ui, |ui| {
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let height = (line_count as f32 * row_height + 8.0).max(ui.available_height());
                    let (gutter, gutter_response) =
                        ui.allocate_exact_size(egui::vec2(gutter_width, height), Sense::click());
                    let output = TextEdit::multiline(&mut document.text)
                        .id(text_id)
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

                    let row_top = |line: usize| {
                        output.galley_pos.y
                            + output
                                .galley
                                .rows
                                .get(line - 1)
                                .map_or((line - 1) as f32 * row_height, |row| row.pos.y)
                    };
                    // The gutter: breakpoints, the paused line and numbers.
                    let painter = ui.painter_at(gutter);
                    let hovered_line = gutter_response.hover_pos().map(|pointer| {
                        (1..=line_count)
                            .find(|&line| pointer.y < row_top(line) + row_height)
                            .unwrap_or(line_count)
                    });
                    for line in 1..=line_count {
                        let top = row_top(line);
                        if top > gutter.bottom() || top + row_height < gutter.top() {
                            continue;
                        }
                        let center = egui::pos2(gutter.left() + 11.0, top + row_height / 2.0);
                        if document.breakpoints.contains(&line) {
                            painter.circle_filled(center, 5.0, palette.danger);
                        } else if hovered_line == Some(line) {
                            painter.circle_stroke(
                                center,
                                5.0,
                                egui::Stroke::new(1.0, palette.danger.gamma_multiply(0.6)),
                            );
                        }
                        if paused_line == Some(line) {
                            let (x, y) = (gutter.left() + 4.0, top + row_height / 2.0);
                            painter.add(egui::Shape::convex_polygon(
                                vec![
                                    egui::pos2(x, y - 6.0),
                                    egui::pos2(x + 13.0, y),
                                    egui::pos2(x, y + 6.0),
                                ],
                                palette.warning,
                                egui::Stroke::NONE,
                            ));
                        }
                        let color = if error_lines.contains(&Some(line)) {
                            palette.danger
                        } else if line == cursor_line {
                            palette.text
                        } else {
                            palette.gutter
                        };
                        painter.text(
                            egui::pos2(gutter.right() - 10.0, top),
                            Align2::RIGHT_TOP,
                            line.to_string(),
                            font.clone(),
                            color,
                        );
                    }
                    if gutter_response.clicked() {
                        clicked_line = hovered_line;
                    }
                    if let Some(line) = scroll_to {
                        let top = row_top(line.clamp(1, line_count));
                        ui.scroll_to_rect(
                            egui::Rect::from_min_size(
                                egui::pos2(gutter.left(), top),
                                egui::vec2(1.0, row_height),
                            ),
                            Some(Align::Center),
                        );
                    }
                    if let Some(index) = cursor {
                        let rect = output.galley.pos_from_cursor(CCursor::new(index));
                        popup_at = Some(output.galley_pos + rect.left_bottom().to_vec2());
                    }
                });
            });

        if changed {
            document.error_line = None;
        }
        if let Some(line) = clicked_line {
            self.toggle_breakpoint(line);
        }
        let Some(index) = cursor else {
            return;
        };
        let text = &self.documents[active].text;
        self.cursor = line_and_column(text, index);
        let moved = self.cursor_index != Some(index);
        self.cursor_index = Some(index);

        // Open, update or close the suggestions.
        let word = completion::word_before(text, index);
        let wanted = match &word {
            Some((_, prefix)) if force_complete => !prefix.is_empty(),
            Some((_, prefix)) if changed => prefix.chars().count() >= 2,
            Some((start, _)) => self
                .completion
                .as_ref()
                .is_some_and(|completion| completion.start == *start && !moved || changed),
            None => false,
        };
        self.completion = match word {
            Some((start, prefix)) if wanted => {
                let lang = earth::detect_language(text);
                let items = completion::suggestions(text, &prefix, lang, SUGGESTIONS);
                let selected = self
                    .completion
                    .as_ref()
                    .filter(|completion| completion.start == start)
                    .map_or(0, |completion| {
                        completion.selected.min(items.len().saturating_sub(1))
                    });
                let rect = self
                    .completion
                    .as_ref()
                    .and_then(|completion| completion.rect);
                (!items.is_empty()).then_some(Completion {
                    start,
                    items,
                    selected,
                    accept: false,
                    rect,
                })
            }
            _ => None,
        };
        if !has_focus && !force_complete {
            // Clicking elsewhere closes the list, except on the list itself.
            let pressed_at = ui.input(|input| {
                input
                    .pointer
                    .any_pressed()
                    .then(|| input.pointer.interact_pos())
                    .flatten()
            });
            let on_list = |position: egui::Pos2| {
                self.completion
                    .as_ref()
                    .and_then(|completion| completion.rect)
                    .is_some_and(|rect| rect.contains(position))
            };
            if pressed_at.is_some_and(|position| !on_list(position)) {
                self.completion = None;
            }
        }
        if let (Some(position), Some(_)) = (popup_at, &self.completion) {
            self.completion_list(ui.ctx(), position);
        }
    }

    fn completion_list(&mut self, context: &egui::Context, position: egui::Pos2) {
        let palette = self.palette();
        let texts = self.texts();
        let Some(completion) = &mut self.completion else {
            return;
        };
        let mut clicked = None;
        let area = egui::Area::new(egui::Id::new("completion"))
            .order(egui::Order::Foreground)
            .fixed_pos(position + egui::vec2(0.0, 2.0))
            .show(context, |ui| {
                egui::Frame::popup(ui.style())
                    .fill(palette.panel)
                    .stroke(egui::Stroke::new(1.0, palette.accent.gamma_multiply(0.6)))
                    .show(ui, |ui| {
                        ui.set_width(250.0);
                        for (index, item) in completion.items.iter().enumerate() {
                            let (color, kind) = match item.kind {
                                Kind::Keyword => (palette.syntax.keyword, texts.kind_keyword),
                                Kind::Builtin => (palette.syntax.builtin, texts.kind_builtin),
                                Kind::Function => (palette.syntax.builtin, texts.kind_function),
                                Kind::Name => (palette.text, texts.kind_name),
                            };
                            let selected = index == completion.selected;
                            let response = ui
                                .horizontal(|ui| {
                                    let label = ui.selectable_label(
                                        selected,
                                        RichText::new(&item.word).monospace().color(color),
                                    );
                                    ui.with_layout(
                                        egui::Layout::right_to_left(Align::Center),
                                        |ui| ui.label(RichText::new(kind).small().weak()),
                                    );
                                    label
                                })
                                .inner;
                            if response.clicked() {
                                clicked = Some(index);
                            }
                        }
                        ui.label(RichText::new(texts.completion_hint).small().weak());
                    });
            });
        completion.rect = Some(area.response.rect);
        if let Some(index) = clicked {
            completion.selected = index;
            completion.accept = true;
            let id = self.text_id();
            context.memory_mut(|memory| memory.request_focus(id));
        }
    }

    /// The bar under the editor that shows the live syntax error.
    pub(super) fn syntax_bar(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette();
        let texts = self.texts();
        let Some(error) = self.documents[self.active].syntax_error.clone() else {
            return;
        };
        let text = texts
            .syntax_error_at
            .replace("{line}", &error.line.to_string())
            .replace("{column}", &error.column.to_string());
        let response = ui
            .add(
                egui::Label::new(
                    RichText::new(format!("⚠ {text}: {}", error.message)).color(palette.danger),
                )
                .sense(Sense::click()),
            )
            .on_hover_text(texts.syntax_error_hint);
        if response.clicked() {
            self.go_to(ui.ctx(), error.line, error.column);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::check;

    #[test]
    fn finds_syntax_errors_with_their_position() {
        assert_eq!(check("yaz(1)\n"), None);
        let error = check("yaz(1)\nyaz(1 +)\n").unwrap();
        assert_eq!((error.line, error.column), (2, 8));
        assert_eq!(error.message, "İfade bekleniyordu, ')' bulundu");
        let english = check("print(1 +)").unwrap();
        assert_eq!(english.message, "Expected an expression, found ')'");
    }
}
