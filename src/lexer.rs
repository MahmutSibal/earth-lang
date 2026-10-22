use crate::error::{Error, Position};
use crate::keywords::{keyword, language_of, Keyword, Lang};
use crate::messages::{Found, Message};
use crate::value::format_number;

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
    DoubleStar,
    Equal,
    PlusEqual,
    MinusEqual,
    StarEqual,
    SlashEqual,
    PercentEqual,
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
    pub fn found(&self) -> Found {
        let symbol = match self {
            TokenKind::Identifier(name) => return Found::Token(name.clone()),
            TokenKind::Keyword(keyword) => return Found::Keyword(*keyword),
            TokenKind::Number(value) => return Found::Token(format_number(*value)),
            TokenKind::String(_) => return Found::Text,
            TokenKind::Newline => return Found::Newline,
            TokenKind::Eof => return Found::Eof,
            TokenKind::LeftParen => "(",
            TokenKind::RightParen => ")",
            TokenKind::LeftBrace => "{",
            TokenKind::RightBrace => "}",
            TokenKind::LeftBracket => "[",
            TokenKind::RightBracket => "]",
            TokenKind::Comma => ",",
            TokenKind::Colon => ":",
            TokenKind::Semicolon => ";",
            TokenKind::Plus => "+",
            TokenKind::Minus => "-",
            TokenKind::Star => "*",
            TokenKind::Slash => "/",
            TokenKind::Percent => "%",
            TokenKind::DoubleStar => "**",
            TokenKind::Equal => "=",
            TokenKind::PlusEqual => "+=",
            TokenKind::MinusEqual => "-=",
            TokenKind::StarEqual => "*=",
            TokenKind::SlashEqual => "/=",
            TokenKind::PercentEqual => "%=",
            TokenKind::EqualEqual => "==",
            TokenKind::BangEqual => "!=",
            TokenKind::Less => "<",
            TokenKind::LessEqual => "<=",
            TokenKind::Greater => ">",
            TokenKind::GreaterEqual => ">=",
        };
        Found::Token(symbol.to_owned())
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct Token {
    pub kind: TokenKind,
    pub position: Position,
    /// The human language of the word, for keywords and built-in names.
    pub lang: Option<Lang>,
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
                    let lang = language_of(&word);
                    let kind = match keyword(&word) {
                        Some(keyword) => TokenKind::Keyword(keyword),
                        None => TokenKind::Identifier(word),
                    };
                    self.tokens.push(Token {
                        kind,
                        position: start,
                        lang,
                    });
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
            '+' if self.eat('=') => TokenKind::PlusEqual,
            '+' => TokenKind::Plus,
            '-' if self.eat('=') => TokenKind::MinusEqual,
            '-' => TokenKind::Minus,
            '*' if self.eat('=') => TokenKind::StarEqual,
            '*' if self.eat('*') => TokenKind::DoubleStar,
            '*' => TokenKind::Star,
            '/' if self.eat('=') => TokenKind::SlashEqual,
            '/' => TokenKind::Slash,
            '%' if self.eat('=') => TokenKind::PercentEqual,
            '%' => TokenKind::Percent,
            '=' if self.eat('=') => TokenKind::EqualEqual,
            '=' => TokenKind::Equal,
            '!' if self.eat('=') => TokenKind::BangEqual,
            '<' if self.eat('=') => TokenKind::LessEqual,
            '<' => TokenKind::Less,
            '>' if self.eat('=') => TokenKind::GreaterEqual,
            '>' => TokenKind::Greater,
            other => return Err(Error::at(start, Message::UnexpectedCharacter(other))),
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
                            return Err(Error::at(escape_position, Message::InvalidEscape(other)))
                        }
                        None => break,
                    }
                }
                character => value.push(character),
            }
        }

        Err(Error::at(start, Message::UnterminatedString))
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
            .map_err(|_| Error::at(start, Message::InvalidNumber(text.clone())))
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
        self.tokens.push(Token {
            kind,
            position,
            lang: None,
        });
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

/// The language most of a program's keywords and built-ins are written in.
/// Ties, and programs without any, count as Turkish.
pub fn detect_language(tokens: &[Token]) -> Lang {
    let english = tokens.iter().filter(|t| t.lang == Some(Lang::En)).count();
    let turkish = tokens.iter().filter(|t| t.lang == Some(Lang::Tr)).count();
    if english > turkish {
        Lang::En
    } else {
        Lang::Tr
    }
}

#[cfg(test)]
mod tests {
    use super::{detect_language, lex, TokenKind};
    use crate::error::Position;
    use crate::keywords::Keyword;
    use crate::keywords::Lang;
    use crate::messages::Message;

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
        assert_eq!(error.message, Message::UnterminatedString);
    }

    #[test]
    fn detects_the_program_language() {
        let detect = |source| detect_language(&lex(source).unwrap());
        assert_eq!(detect("if true { print(\"eğer\") }"), Lang::En);
        assert_eq!(detect("eğer doğru { yaz(\"if\") }"), Lang::Tr);
        assert_eq!(detect("x = 1"), Lang::Tr);
    }
}
