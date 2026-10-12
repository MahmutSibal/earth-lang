//! Syntax highlighting for the editor. Words are classified with Earth's own
//! keyword and built-in tables, so every spelling in every language is
//! coloured the same way the interpreter understands it.

use earth::keywords::{keyword, language_of, Keyword};
use eframe::egui::text::{LayoutJob, TextFormat};
use eframe::egui::{Color32, FontId, Stroke};

pub struct Theme {
    pub text: Color32,
    pub keyword: Color32,
    pub constant: Color32,
    pub builtin: Color32,
    pub string: Color32,
    pub number: Color32,
    pub comment: Color32,
    pub error_background: Color32,
    pub paused_background: Color32,
    pub squiggle: Color32,
}

pub const DARK: Theme = Theme {
    text: Color32::from_rgb(0xd4, 0xd4, 0xd4),
    keyword: Color32::from_rgb(0xc5, 0x86, 0xc0),
    constant: Color32::from_rgb(0x56, 0x9c, 0xd6),
    builtin: Color32::from_rgb(0xdc, 0xdc, 0xaa),
    string: Color32::from_rgb(0xce, 0x91, 0x78),
    number: Color32::from_rgb(0xb5, 0xce, 0xa8),
    comment: Color32::from_rgb(0x6a, 0x99, 0x55),
    error_background: Color32::from_rgba_premultiplied(0x5a, 0x1d, 0x1d, 0xff),
    paused_background: Color32::from_rgb(0x4b, 0x47, 0x1a),
    squiggle: Color32::from_rgb(0xf4, 0x47, 0x47),
};

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Text,
    Keyword,
    Constant,
    Builtin,
    String,
    Number,
    Comment,
}

/// Splits `source` into byte ranges with a highlighting kind each.
fn classify(source: &str) -> Vec<(std::ops::Range<usize>, Kind)> {
    let mut spans: Vec<(std::ops::Range<usize>, Kind)> = Vec::new();
    let mut chars = source.char_indices().peekable();
    while let Some((start, c)) = chars.next() {
        let kind = match c {
            '#' => {
                while chars.peek().is_some_and(|&(_, c)| c != '\n') {
                    chars.next();
                }
                Kind::Comment
            }
            '"' => {
                let mut escaped = false;
                for (_, c) in chars.by_ref() {
                    match c {
                        _ if escaped => escaped = false,
                        '\\' => escaped = true,
                        '"' | '\n' => break,
                        _ => {}
                    }
                }
                Kind::String
            }
            c if c.is_ascii_digit() => {
                while chars
                    .peek()
                    .is_some_and(|&(_, c)| c.is_ascii_digit() || c == '.')
                {
                    chars.next();
                }
                Kind::Number
            }
            c if c.is_alphabetic() || c == '_' => {
                while chars
                    .peek()
                    .is_some_and(|&(_, c)| c.is_alphanumeric() || c == '_')
                {
                    chars.next();
                }
                let end = chars.peek().map_or(source.len(), |&(i, _)| i);
                match keyword(&source[start..end]) {
                    Some(Keyword::True | Keyword::False | Keyword::Nil) => Kind::Constant,
                    Some(_) => Kind::Keyword,
                    None if language_of(&source[start..end]).is_some() => Kind::Builtin,
                    None => Kind::Text,
                }
            }
            _ => Kind::Text,
        };
        let end = chars.peek().map_or(source.len(), |&(i, _)| i);
        match spans.last_mut() {
            Some((range, last)) if *last == kind && kind == Kind::Text => range.end = end,
            _ => spans.push((start..end, kind)),
        }
    }
    spans
}

/// What to mark in the editor besides the syntax colours. Lines are
/// 1-based; a column is 1-based too, in characters.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Marks {
    /// The line a run stopped with an error on: red background.
    pub error_line: Option<usize>,
    /// The line the debugger is paused on: highlighted background.
    pub paused_line: Option<usize>,
    /// A syntax error found while typing: underlined from its column to
    /// the end of its line.
    pub squiggle: Option<(usize, usize)>,
}

/// Builds the coloured text for the editor.
pub fn layout(source: &str, font: FontId, theme: &Theme, marks: &Marks) -> LayoutJob {
    // Byte ranges with a background or an underline.
    let mut backgrounds = Vec::new();
    if let Some(range) = marks.paused_line.and_then(|line| line_range(source, line)) {
        backgrounds.push((range, theme.paused_background));
    }
    if let Some(range) = marks.error_line.and_then(|line| line_range(source, line)) {
        backgrounds.push((range, theme.error_background));
    }
    let squiggle = marks.squiggle.and_then(|(line, column)| {
        let range = line_range(source, line)?;
        let text = &source[range.clone()];
        let start = text
            .char_indices()
            .nth(column.saturating_sub(1))
            .map_or(text.len(), |(index, _)| index);
        // An error at the very end of a line marks its last character.
        let start = if start == text.len() {
            text.char_indices().last().map_or(0, |(index, _)| index)
        } else {
            start
        };
        (range.start + start < range.end).then_some(range.start + start..range.end)
    });

    let mut job = LayoutJob::default();
    for (range, kind) in classify(source) {
        let color = match kind {
            Kind::Text => theme.text,
            Kind::Keyword => theme.keyword,
            Kind::Constant => theme.constant,
            Kind::Builtin => theme.builtin,
            Kind::String => theme.string,
            Kind::Number => theme.number,
            Kind::Comment => theme.comment,
        };
        // Split spans at the edges of marked ranges so only they are marked.
        let mut cuts = vec![range.start, range.end];
        let edges = backgrounds
            .iter()
            .map(|(marked, _)| marked)
            .chain(squiggle.as_ref());
        for marked in edges {
            for edge in [marked.start, marked.end] {
                if range.start < edge && edge < range.end {
                    cuts.push(edge);
                }
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        for pair in cuts.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let inside =
                |marked: &std::ops::Range<usize>| start >= marked.start && end <= marked.end;
            let background = backgrounds
                .iter()
                .rev()
                .find(|(marked, _)| inside(marked))
                .map_or(Color32::TRANSPARENT, |(_, color)| *color);
            let underline = if squiggle.as_ref().is_some_and(inside) {
                Stroke::new(1.5, theme.squiggle)
            } else {
                Stroke::NONE
            };
            let format = TextFormat {
                font_id: font.clone(),
                color,
                background,
                underline,
                ..Default::default()
            };
            job.append(&source[start..end], 0.0, format);
        }
    }
    job
}

/// The byte range of a 1-based line, without its newline.
fn line_range(source: &str, line: usize) -> Option<std::ops::Range<usize>> {
    let mut start = 0;
    for (index, text) in source.split('\n').enumerate() {
        if index + 1 == line {
            return Some(start..start + text.len());
        }
        start += text.len() + 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{classify, line_range, Kind};

    fn kinds(source: &str) -> Vec<(&str, Kind)> {
        classify(source)
            .into_iter()
            .filter(|(_, kind)| *kind != Kind::Text)
            .map(|(range, kind)| (&source[range], kind))
            .collect()
    }

    #[test]
    fn classifies_words_in_every_language() {
        assert_eq!(
            kinds("eğer x { yaz(\"a#b\", 12.5, doğru) } # not\nwhile nil"),
            vec![
                ("eğer", Kind::Keyword),
                ("yaz", Kind::Builtin),
                ("\"a#b\"", Kind::String),
                ("12.5", Kind::Number),
                ("doğru", Kind::Constant),
                ("# not", Kind::Comment),
                ("while", Kind::Keyword),
                ("nil", Kind::Constant),
            ]
        );
    }

    #[test]
    fn marks_lines_and_underlines_errors() {
        use super::{layout, Marks, DARK};
        use eframe::egui::FontId;
        let marks = Marks {
            error_line: None,
            paused_line: Some(1),
            squiggle: Some((2, 5)),
        };
        let job = layout(
            "yaz(1)
yaz(1 +)",
            FontId::monospace(14.0),
            &DARK,
            &marks,
        );
        let text = |section: &eframe::egui::text::LayoutSection| {
            &job.text[section.byte_range.start.0..section.byte_range.end.0]
        };
        let paused: String = job
            .sections
            .iter()
            .filter(|s| s.format.background == DARK.paused_background)
            .map(text)
            .collect();
        assert_eq!(paused, "yaz(1)");
        let underlined: String = job
            .sections
            .iter()
            .filter(|s| s.format.underline.width > 0.0)
            .map(text)
            .collect();
        assert_eq!(underlined, "1 +)");
    }

    #[test]
    fn finds_line_ranges() {
        assert_eq!(line_range("ab\ncd\n", 2), Some(3..5));
        assert_eq!(line_range("ab", 3), None);
    }
}
