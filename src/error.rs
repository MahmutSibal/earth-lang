use std::fmt;
use std::path::{Path, PathBuf};

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
    /// The imported file the error happened in; `None` for the main program.
    pub file: Option<PathBuf>,
}

impl Error {
    pub fn at(position: Position, message: Message) -> Self {
        Self {
            message,
            position: Some(position),
            file: None,
        }
    }

    /// Records that the error happened in `file`, unless a more deeply
    /// imported file was already recorded.
    pub fn in_file(mut self, file: &Path) -> Self {
        self.file.get_or_insert_with(|| file.to_path_buf());
        self
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
