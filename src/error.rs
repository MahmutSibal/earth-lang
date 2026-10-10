use std::fmt;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    pub position: Option<Position>,
}

impl Error {
    pub fn at(position: Position, message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position: Some(position),
        }
    }

    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            position: None,
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.position {
            Some(position) => write!(f, "{position}: {}", self.message),
            None => write!(f, "{}", self.message),
        }
    }
}

impl std::error::Error for Error {}
