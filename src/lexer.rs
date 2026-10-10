#[derive(Debug, PartialEq, Eq, Clone)]
pub enum Token {
    Identifier(String),
    String(String),
    LeftParen,
    RightParen,
    Comma,
}

pub fn lex(source: &str) -> Result<Vec<Token>, String> {
    let mut chars = source.chars().peekable();
    let mut tokens = Vec::new();

    while let Some(&character) = chars.peek() {
        match character {
            whitespace if whitespace.is_whitespace() => {
                chars.next();
            }
            '(' => {
                chars.next();
                tokens.push(Token::LeftParen);
            }
            ')' => {
                chars.next();
                tokens.push(Token::RightParen);
            }
            ',' => {
                chars.next();
                tokens.push(Token::Comma);
            }
            '"' => tokens.push(Token::String(read_string(&mut chars)?)),
            character if character.is_alphabetic() || character == '_' => {
                let mut identifier = String::new();
                while let Some(&character) = chars.peek() {
                    if character.is_alphanumeric() || character == '_' {
                        identifier.push(character);
                        chars.next();
                    } else {
                        break;
                    }
                }
                tokens.push(Token::Identifier(identifier));
            }
            character => {
                return Err(format!("Unexpected character '{character}'"));
            }
        }
    }

    Ok(tokens)
}

fn read_string<I>(chars: &mut std::iter::Peekable<I>) -> Result<String, String>
where
    I: Iterator<Item = char>,
{
    chars.next();
    let mut value = String::new();

    while let Some(character) = chars.next() {
        match character {
            '"' => return Ok(value),
            '\\' => match chars.next() {
                Some('"') => value.push('"'),
                Some('\\') => value.push('\\'),
                Some('n') => value.push('\n'),
                Some('r') => value.push('\r'),
                Some('t') => value.push('\t'),
                Some(escaped) => return Err(format!("Invalid escape '\\{escaped}'")),
                None => return Err("Unterminated string escape".to_owned()),
            },
            character => value.push(character),
        }
    }

    Err("Unterminated string".to_owned())
}

#[cfg(test)]
mod tests {
    use super::{lex, Token};

    #[test]
    fn lexes_quoted_strings_and_escapes() {
        assert_eq!(
            lex(r#"print("Hello, \"Earth\"!")"#).unwrap(),
            vec![
                Token::Identifier("print".to_owned()),
                Token::LeftParen,
                Token::String("Hello, \"Earth\"!".to_owned()),
                Token::RightParen,
            ]
        );
    }

    #[test]
    fn rejects_unterminated_strings() {
        assert_eq!(
            lex(r#"print("missing)"#),
            Err("Unterminated string".to_owned())
        );
    }
}
