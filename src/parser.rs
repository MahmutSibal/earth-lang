use crate::ast::{Program, Statement};
use crate::lexer::Token;

pub fn parse(tokens: &[Token]) -> Result<Program, String> {
    let mut parser = Parser {
        tokens,
        position: 0,
    };
    let mut statements = Vec::new();

    while parser.position < tokens.len() {
        statements.push(parser.parse_print_statement()?);
    }

    Ok(Program { statements })
}

struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
}

impl Parser<'_> {
    fn parse_print_statement(&mut self) -> Result<Statement, String> {
        let name = match self.next() {
            Some(Token::Identifier(name)) => name,
            Some(token) => return Err(format!("Expected print function, found {token:?}")),
            None => return Err("Expected print function".to_owned()),
        };

        if name != "yaz" && name != "print" {
            return Err(format!("Unknown function '{name}'"));
        }
        self.expect(Token::LeftParen, "Expected '(' after print function")?;
        let value = match self.next() {
            Some(Token::String(value)) => value.clone(),
            Some(token) => return Err(format!("Expected quoted string, found {token:?}")),
            None => return Err("Expected quoted string".to_owned()),
        };
        self.expect(Token::RightParen, "Expected ')' after quoted string")?;

        Ok(Statement::Print(value))
    }

    fn next(&mut self) -> Option<&Token> {
        let token = self.tokens.get(self.position);
        self.position += usize::from(token.is_some());
        token
    }

    fn expect(&mut self, expected: Token, message: &str) -> Result<(), String> {
        match self.next() {
            Some(token) if *token == expected => Ok(()),
            _ => Err(message.to_owned()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::ast::{Program, Statement};
    use crate::lexer::lex;

    #[test]
    fn parses_multiple_print_statements() {
        let tokens = lex("yaz(\"Merhaba\") print(\"Hello\")").unwrap();
        assert_eq!(
            parse(&tokens).unwrap(),
            Program {
                statements: vec![
                    Statement::Print("Merhaba".to_owned()),
                    Statement::Print("Hello".to_owned()),
                ],
            }
        );
    }

    #[test]
    fn rejects_missing_parenthesis() {
        let tokens = lex("print(\"Hello\"").unwrap();
        assert_eq!(
            parse(&tokens),
            Err("Expected ')' after quoted string".to_owned())
        );
    }
}
