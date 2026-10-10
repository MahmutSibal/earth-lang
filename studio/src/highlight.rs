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

/// Builds the coloured text for the editor. `error_line` (1-based) gets a
/// red background.
pub fn layout(source: &str, font: FontId, theme: &Theme, error_line: Option<usize>) -> LayoutJob {
    let mut job = LayoutJob::default();
    let error_range = error_line.and_then(|line| line_range(source, line));
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
        // Split spans at the edges of the error line so only it is marked.
        let mut cuts = vec![range.start, range.end];
        if let Some(error) = &error_range {
            for edge in [error.start, error.end] {
                if range.start < edge && edge < range.end {
                    cuts.push(edge);
                }
            }
        }
        cuts.sort_unstable();
        for pair in cuts.windows(2) {
            let (start, end) = (pair[0], pair[1]);
            let on_error_line = error_range
                .as_ref()
                .is_some_and(|error| start >= error.start && end <= error.end);
            let format = TextFormat {
                font_id: font.clone(),
                color,
                background: if on_error_line {
                    theme.error_background
                } else {
                    Color32::TRANSPARENT
                },
                underline: Stroke::NONE,
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
    fn finds_line_ranges() {
        assert_eq!(line_range("ab\ncd\n", 2), Some(3..5));
        assert_eq!(line_range("ab", 3), None);
    }
}
