use std::fmt;

use crate::keywords::Lang;
use crate::messages::Message;

/// A 1-based line and column in the source file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

impl Position {
    pub fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }
}

impl fmt::Display for Position {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.line, self.column)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Error {
    pub message: Message,
    pub position: Option<Position>,
}

impl Error {
    pub fn at(position: Position, message: Message) -> Self {
        Self {
            message,
            position: Some(position),
        }
    }

    /// The message alone, in `lang`.
    pub fn text(&self, lang: Lang) -> String {
        self.message.render(lang)
    }

    /// `line:column: message`, in `lang`.
    pub fn render(&self, lang: Lang) -> String {
        match self.position {
            Some(position) => format!("{position}: {}", self.text(lang)),
            None => self.text(lang),
        }
    }
}

/// Shows the error in Turkish; use [`Error::render`] for other languages.
impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.render(Lang::Tr))
    }
}

impl std::error::Error for Error {}
