use crate::error::{Error, Position};
use crate::keywords::{keyword, Keyword};

#[derive(Debug, PartialEq, Clone)]
pub enum TokenKind {
    Identifier(String),
    Keyword(Keyword),
    Number(f64),
    String(String),
    LeftParen,
    RightParen,
    LeftBrace,
    RightBrace,
    LeftBracket,
    RightBracket,
    Comma,
    Colon,
    Semicolon,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Equal,
    EqualEqual,
    BangEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Newline,
    Eof,
}

impl TokenKind {
    /// How the token is shown to the user in error messages.
    pub fn describe(&self) -> String {
        match self {
            TokenKind::Identifier(name) => format!("'{name}'"),
            TokenKind::Keyword(keyword) => format!("'{}'", keyword.name()),
            TokenKind::Number(value) => format!("'{value}'"),
            TokenKind::String(_) => "metin".to_owned(),
            TokenKind::LeftParen => "'('".to_owned(),
            TokenKind::RightParen => "')'".to_owned(),
            TokenKind::LeftBrace => "'{'".to_owned(),
            TokenKind::RightBrace => "'}'".to_owned(),
            TokenKind::LeftBracket => "'['".to_owned(),
            TokenKind::RightBracket => "']'".to_owned(),
            TokenKind::Comma => "','".to_owned(),
            TokenKind::Colon => "':'".to_owned(),
            TokenKind::Semicolon => "';'".to_owned(),
            TokenKind::Plus => "'+'".to_owned(),
            TokenKind::Minus => "'-'".to_owned(),
            TokenKind::Star => "'*'".to_owned(),
            TokenKind::Slash => "'/'".to_owned(),
            TokenKind::Percent => "'%'".to_owned(),
            TokenKind::Equal => "'='".to_owned(),
            TokenKind::EqualEqual => "'=='".to_owned(),
            TokenKind::BangEqual => "'!='".to_owned(),
            TokenKind::Less => "'<'".to_owned(),
            TokenKind::LessEqual => "'<='".to_owned(),
            TokenKind::Greater => "'>'".to_owned(),
            TokenKind::GreaterEqual => "'>='".to_owned(),
            TokenKind::Newline => "satır sonu".to_owned(),
            TokenKind::Eof => "dosya sonu".to_owned(),
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub position: Position,
}

/// Converts source text into tokens. The result always ends with `Eof`.
///
/// Newlines separate statements, so they are emitted as tokens, except inside
/// parentheses and brackets where an expression may span several lines.
pub fn lex(source: &str) -> Result<Vec<Token>, Error> {
    Lexer {
        chars: source.chars().collect(),
        index: 0,
        line: 1,
        column: 1,
        nesting: 0,
        tokens: Vec::new(),
    }
    .run()
}

struct Lexer {
    chars: Vec<char>,
    index: usize,
    line: usize,
    column: usize,
    nesting: usize,
    tokens: Vec<Token>,
}

impl Lexer {
    fn run(mut self) -> Result<Vec<Token>, Error> {
        while let Some(character) = self.peek() {
            let start = self.position();
            match character {
                '\n' => {
                    self.advance();
                    if self.nesting == 0 {
                        self.push(TokenKind::Newline, start);
                    }
                }
                whitespace if whitespace.is_whitespace() => {
                    self.advance();
                }
                '#' => {
                    while self.peek().is_some_and(|c| c != '\n') {
                        self.advance();
                    }
                }
                '"' => {
                    let value = self.read_string()?;
                    self.push(TokenKind::String(value), start);
                }
                digit if digit.is_ascii_digit() => {
                    let value = self.read_number()?;
                    self.push(TokenKind::Number(value), start);
                }
                letter if letter.is_alphabetic() || letter == '_' => {
                    let word = self.read_word();
                    let kind = match keyword(&word) {
                        Some(keyword) => TokenKind::Keyword(keyword),
                        None => TokenKind::Identifier(word),
                    };
                    self.push(kind, start);
                }
                _ => {
                    self.advance();
                    let kind = self.read_symbol(character, start)?;
                    self.push(kind, start);
                }
            }
        }

        let end = self.position();
        self.push(TokenKind::Eof, end);
        Ok(self.tokens)
    }

    fn read_symbol(&mut self, character: char, start: Position) -> Result<TokenKind, Error> {
        let kind = match character {
            '(' => {
                self.nesting += 1;
                TokenKind::LeftParen
            }
            ')' => {
                self.nesting = self.nesting.saturating_sub(1);
                TokenKind::RightParen
            }
            '[' => {
                self.nesting += 1;
                TokenKind::LeftBracket
            }
            ']' => {
                self.nesting = self.nesting.saturating_sub(1);
                TokenKind::RightBracket
            }
            '{' => TokenKind::LeftBrace,
            '}' => TokenKind::RightBrace,
            ',' => TokenKind::Comma,
            ':' => TokenKind::Colon,
            ';' => TokenKind::Semicolon,
            '+' => TokenKind::Plus,
            '-' => TokenKind::Minus,
            '*' => TokenKind::Star,
            '/' => TokenKind::Slash,
            '%' => TokenKind::Percent,
            '=' if self.eat('=') => TokenKind::EqualEqual,
            '=' => TokenKind::Equal,
            '!' if self.eat('=') => TokenKind::BangEqual,
            '<' if self.eat('=') => TokenKind::LessEqual,
            '<' => TokenKind::Less,
            '>' if self.eat('=') => TokenKind::GreaterEqual,
            '>' => TokenKind::Greater,
            other => return Err(Error::at(start, format!("Beklenmeyen karakter '{other}'"))),
        };
        Ok(kind)
    }

    fn read_string(&mut self) -> Result<String, Error> {
        let start = self.position();
        self.advance();
        let mut value = String::new();

        while let Some(character) = self.advance() {
            match character {
                '"' => return Ok(value),
                '\\' => {
                    let escape_position = self.position();
                    match self.advance() {
                        Some('"') => value.push('"'),
                        Some('\\') => value.push('\\'),
                        Some('n') => value.push('\n'),
                        Some('r') => value.push('\r'),
                        Some('t') => value.push('\t'),
                        Some(other) => {
                            return Err(Error::at(
                                escape_position,
                                format!("Geçersiz kaçış dizisi '\\{other}'"),
                            ))
                        }
                        None => break,
                    }
                }
                character => value.push(character),
            }
        }

        Err(Error::at(start, "Kapatılmamış metin"))
    }

    fn read_number(&mut self) -> Result<f64, Error> {
        let start = self.position();
        let mut text = String::new();
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            text.push(self.advance().unwrap_or_default());
        }
        if self.peek() == Some('.') && self.peek_next().is_some_and(|c| c.is_ascii_digit()) {
            text.push(self.advance().unwrap_or_default());
            while self.peek().is_some_and(|c| c.is_ascii_digit()) {
                text.push(self.advance().unwrap_or_default());
            }
        }
        text.parse()
            .map_err(|_| Error::at(start, format!("Geçersiz sayı '{text}'")))
    }

    fn read_word(&mut self) -> String {
        let mut word = String::new();
        while let Some(character) = self.peek() {
            if character.is_alphanumeric() || character == '_' {
                word.push(character);
                self.advance();
            } else {
                break;
            }
        }
        word
    }

    fn push(&mut self, kind: TokenKind, position: Position) {
        self.tokens.push(Token { kind, position });
    }

    fn position(&self) -> Position {
        Position::new(self.line, self.column)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_next(&self) -> Option<char> {
        self.chars.get(self.index + 1).copied()
    }

    fn eat(&mut self, expected: char) -> bool {
        if self.peek() == Some(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn advance(&mut self) -> Option<char> {
        let character = self.peek()?;
        self.index += 1;
        if character == '\n' {
            self.line += 1;
            self.column = 1;
        } else {
            self.column += 1;
        }
        Some(character)
    }
}

#[cfg(test)]
mod tests {
    use super::{lex, TokenKind};
    use crate::error::Position;
    use crate::keywords::Keyword;

    fn kinds(source: &str) -> Vec<TokenKind> {
        lex(source)
            .unwrap()
            .into_iter()
            .map(|token| token.kind)
            .collect()
    }

    #[test]
    fn lexes_quoted_strings_and_escapes() {
        assert_eq!(
            kinds(r#"print("Hello, \"Earth\"!")"#),
            vec![
                TokenKind::Identifier("print".to_owned()),
                TokenKind::LeftParen,
                TokenKind::String("Hello, \"Earth\"!".to_owned()),
                TokenKind::RightParen,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn lexes_keywords_numbers_and_operators() {
        assert_eq!(
            kinds("eğer x >= 2.5 # yorum\n"),
            vec![
                TokenKind::Keyword(Keyword::If),
                TokenKind::Identifier("x".to_owned()),
                TokenKind::GreaterEqual,
                TokenKind::Number(2.5),
                TokenKind::Newline,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn ignores_newlines_inside_brackets() {
        assert_eq!(
            kinds("[1,\n2]"),
            vec![
                TokenKind::LeftBracket,
                TokenKind::Number(1.0),
                TokenKind::Comma,
                TokenKind::Number(2.0),
                TokenKind::RightBracket,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn reports_unterminated_strings_with_position() {
        let error = lex("yaz(1)\nprint(\"missing)").unwrap_err();
        assert_eq!(error.position, Some(Position::new(2, 7)));
        assert_eq!(error.message, "Kapatılmamış metin");
    }
}
