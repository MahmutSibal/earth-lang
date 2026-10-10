use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, FunctionDecl, LogicalOp, Program, Stmt, UnaryOp};
use crate::error::{Error, Position};
use crate::keywords::Keyword;
use crate::lexer::{Token, TokenKind};
use crate::messages::{Expected, Message};

/// Parses a token stream produced by [`crate::lexer::lex`].
pub fn parse(tokens: &[Token]) -> Result<Program, Error> {
    let mut parser = Parser {
        tokens,
        position: 0,
        loop_depth: 0,
        function_depth: 0,
    };
    let mut statements = Vec::new();

    parser.skip_separators();
    while !parser.check(&TokenKind::Eof) {
        statements.push(parser.statement()?);
        parser.end_statement()?;
        parser.skip_separators();
    }

    Ok(Program { statements })
}

struct Parser<'a> {
    tokens: &'a [Token],
    position: usize,
    loop_depth: usize,
    function_depth: usize,
}

impl Parser<'_> {
    // ----- statements -----

    fn statement(&mut self) -> Result<Stmt, Error> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Keyword(Keyword::Let) => self.let_statement(),
            TokenKind::Keyword(Keyword::If) => self.if_statement(),
            TokenKind::Keyword(Keyword::While) => self.while_statement(),
            TokenKind::Keyword(Keyword::For) => self.for_statement(token.position),
            TokenKind::Keyword(Keyword::Function) => self.function_statement(),
            TokenKind::Keyword(Keyword::Try) => self.try_statement(),
            TokenKind::Keyword(Keyword::Import) => {
                let path_token = self.advance().clone();
                match path_token.kind {
                    TokenKind::String(path) => Ok(Stmt::Import {
                        path,
                        position: token.position,
                    }),
                    other => Err(Error::at(
                        path_token.position,
                        Message::Expected {
                            expected: Expected::FilePath,
                            found: other.found(),
                        },
                    )),
                }
            }
            TokenKind::Keyword(Keyword::Throw) => Ok(Stmt::Throw {
                value: self.expression()?,
                position: token.position,
            }),
            TokenKind::Keyword(Keyword::Return) => {
                if self.function_depth == 0 {
                    return Err(Error::at(
                        token.position,
                        Message::OutsideFunction(Keyword::Return),
                    ));
                }
                if self.at_statement_end() {
                    Ok(Stmt::Return(None))
                } else {
                    Ok(Stmt::Return(Some(self.expression()?)))
                }
            }
            TokenKind::Keyword(keyword @ (Keyword::Break | Keyword::Continue)) => {
                if self.loop_depth == 0 {
                    return Err(Error::at(token.position, Message::OutsideLoop(keyword)));
                }
                Ok(if keyword == Keyword::Break {
                    Stmt::Break
                } else {
                    Stmt::Continue
                })
            }
            _ => {
                self.position -= 1;
                self.expression_statement()
            }
        }
    }

    fn let_statement(&mut self) -> Result<Stmt, Error> {
        let name = self.identifier(Expected::VariableName)?;
        self.expect(TokenKind::Equal)?;
        let value = self.expression()?;
        Ok(Stmt::Let { name, value })
    }

    fn if_statement(&mut self) -> Result<Stmt, Error> {
        let condition = self.expression()?;
        let then_branch = self.block()?;

        let save = self.position;
        self.skip_separators();
        let else_branch = if self.eat(&TokenKind::Keyword(Keyword::Else)) {
            if self.eat(&TokenKind::Keyword(Keyword::If)) {
                Some(vec![self.if_statement()?])
            } else {
                Some(self.block()?)
            }
        } else {
            self.position = save;
            None
        };

        Ok(Stmt::If {
            condition,
            then_branch,
            else_branch,
        })
    }

    fn try_statement(&mut self) -> Result<Stmt, Error> {
        let body = self.block()?;
        self.skip_separators();
        self.expect(TokenKind::Keyword(Keyword::Catch))?;
        let variable = match &self.peek().kind {
            TokenKind::Identifier(name) => {
                let name = name.clone();
                self.advance();
                Some(name)
            }
            _ => None,
        };
        let handler = self.block()?;
        Ok(Stmt::Try {
            body,
            variable,
            handler,
        })
    }

    fn while_statement(&mut self) -> Result<Stmt, Error> {
        let condition = self.expression()?;
        let body = self.loop_body()?;
        Ok(Stmt::While { condition, body })
    }

    fn for_statement(&mut self, position: Position) -> Result<Stmt, Error> {
        let variable = self.identifier(Expected::LoopVariable)?;
        self.expect(TokenKind::Keyword(Keyword::In))?;
        let iterable = self.expression()?;
        let body = self.loop_body()?;
        Ok(Stmt::For {
            variable,
            iterable,
            body,
            position,
        })
    }

    fn loop_body(&mut self) -> Result<Vec<Stmt>, Error> {
        self.loop_depth += 1;
        let body = self.block();
        self.loop_depth -= 1;
        body
    }

    fn function_statement(&mut self) -> Result<Stmt, Error> {
        let name = self.identifier(Expected::FunctionName)?;
        self.expect(TokenKind::LeftParen)?;
        let mut parameters = Vec::new();
        if !self.check(&TokenKind::RightParen) {
            loop {
                let parameter_position = self.peek().position;
                let parameter = self.identifier(Expected::ParameterName)?;
                if parameters.contains(&parameter) {
                    return Err(Error::at(
                        parameter_position,
                        Message::DuplicateParameter(parameter),
                    ));
                }
                parameters.push(parameter);
                if !self.eat(&TokenKind::Comma) {
                    break;
                }
            }
        }
        self.expect(TokenKind::RightParen)?;

        // A function body starts a fresh context: loops outside it do not
        // make `kır` valid inside it.
        let outer_loop_depth = std::mem::take(&mut self.loop_depth);
        self.function_depth += 1;
        let body = self.block();
        self.function_depth -= 1;
        self.loop_depth = outer_loop_depth;

        Ok(Stmt::Function(Rc::new(FunctionDecl {
            name,
            parameters,
            body: body?,
        })))
    }

    fn expression_statement(&mut self) -> Result<Stmt, Error> {
        let expression = self.expression()?;
        if !self.check(&TokenKind::Equal) {
            return Ok(Stmt::Expression(expression));
        }
        let position = self.advance().position;
        let value = self.expression()?;
        match expression {
            Expr::Variable { name, .. } => Ok(Stmt::Assign {
                name,
                value,
                position,
            }),
            Expr::Index { target, index, .. } => Ok(Stmt::IndexAssign {
                target: *target,
                index: *index,
                value,
                position,
            }),
            _ => Err(Error::at(position, Message::InvalidAssignmentTarget)),
        }
    }

    fn block(&mut self) -> Result<Vec<Stmt>, Error> {
        self.expect(TokenKind::LeftBrace)?;
        let mut statements = Vec::new();
        self.skip_separators();
        while !self.check(&TokenKind::RightBrace) {
            if self.check(&TokenKind::Eof) {
                return Err(Error::at(self.peek().position, Message::UnclosedBlock));
            }
            statements.push(self.statement()?);
            self.end_statement()?;
            self.skip_separators();
        }
        self.advance();
        Ok(statements)
    }

    /// A statement must be followed by a newline, `;`, `}` or the end of the
    /// file, unless it ended with a block.
    fn end_statement(&mut self) -> Result<(), Error> {
        if self.at_statement_end() || self.previous_kind() == Some(&TokenKind::RightBrace) {
            return Ok(());
        }
        let token = self.peek();
        Err(Error::at(
            token.position,
            Message::ExpectedStatementEnd(token.kind.found()),
        ))
    }

    fn at_statement_end(&self) -> bool {
        matches!(
            self.peek().kind,
            TokenKind::Newline | TokenKind::Semicolon | TokenKind::RightBrace | TokenKind::Eof
        )
    }

    fn skip_separators(&mut self) {
        while matches!(self.peek().kind, TokenKind::Newline | TokenKind::Semicolon) {
            self.advance();
        }
    }

    // ----- expressions, lowest precedence first -----

    fn expression(&mut self) -> Result<Expr, Error> {
        self.or()
    }

    fn or(&mut self) -> Result<Expr, Error> {
        let mut left = self.and()?;
        while self.eat(&TokenKind::Keyword(Keyword::Or)) {
            let right = self.and()?;
            left = Expr::Logical {
                op: LogicalOp::Or,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn and(&mut self) -> Result<Expr, Error> {
        let mut left = self.equality()?;
        while self.eat(&TokenKind::Keyword(Keyword::And)) {
            let right = self.equality()?;
            left = Expr::Logical {
                op: LogicalOp::And,
                left: Box::new(left),
                right: Box::new(right),
            };
        }
        Ok(left)
    }

    fn equality(&mut self) -> Result<Expr, Error> {
        self.binary_level(Self::comparison, |kind| match kind {
            TokenKind::EqualEqual => Some(BinaryOp::Equal),
            TokenKind::BangEqual => Some(BinaryOp::NotEqual),
            _ => None,
        })
    }

    fn comparison(&mut self) -> Result<Expr, Error> {
        self.binary_level(Self::term, |kind| match kind {
            TokenKind::Less => Some(BinaryOp::Less),
            TokenKind::LessEqual => Some(BinaryOp::LessEqual),
            TokenKind::Greater => Some(BinaryOp::Greater),
            TokenKind::GreaterEqual => Some(BinaryOp::GreaterEqual),
            _ => None,
        })
    }

    fn term(&mut self) -> Result<Expr, Error> {
        self.binary_level(Self::factor, |kind| match kind {
            TokenKind::Plus => Some(BinaryOp::Add),
            TokenKind::Minus => Some(BinaryOp::Subtract),
            _ => None,
        })
    }

    fn factor(&mut self) -> Result<Expr, Error> {
        self.binary_level(Self::unary, |kind| match kind {
            TokenKind::Star => Some(BinaryOp::Multiply),
            TokenKind::Slash => Some(BinaryOp::Divide),
            TokenKind::Percent => Some(BinaryOp::Remainder),
            _ => None,
        })
    }

    fn binary_level(
        &mut self,
        operand: fn(&mut Self) -> Result<Expr, Error>,
        operator: fn(&TokenKind) -> Option<BinaryOp>,
    ) -> Result<Expr, Error> {
        let mut left = operand(self)?;
        while let Some(op) = operator(&self.peek().kind) {
            let position = self.advance().position;
            let right = operand(self)?;
            left = Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
                position,
            };
        }
        Ok(left)
    }

    fn unary(&mut self) -> Result<Expr, Error> {
        let op = match self.peek().kind {
            TokenKind::Minus => UnaryOp::Negate,
            TokenKind::Keyword(Keyword::Not) => UnaryOp::Not,
            _ => return self.postfix(),
        };
        let position = self.advance().position;
        let operand = self.unary()?;
        Ok(Expr::Unary {
            op,
            operand: Box::new(operand),
            position,
        })
    }

    fn postfix(&mut self) -> Result<Expr, Error> {
        let mut expression = self.primary()?;
        loop {
            if self.check(&TokenKind::LeftParen) {
                let position = self.advance().position;
                let arguments = self.list_items(TokenKind::RightParen)?;
                expression = Expr::Call {
                    callee: Box::new(expression),
                    arguments,
                    position,
                };
            } else if self.check(&TokenKind::LeftBracket) {
                let position = self.advance().position;
                let index = self.expression()?;
                self.expect(TokenKind::RightBracket)?;
                expression = Expr::Index {
                    target: Box::new(expression),
                    index: Box::new(index),
                    position,
                };
            } else {
                return Ok(expression);
            }
        }
    }

    fn primary(&mut self) -> Result<Expr, Error> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Number(value) => Ok(Expr::Number(value)),
            TokenKind::String(value) => Ok(Expr::String(value)),
            TokenKind::Keyword(Keyword::True) => Ok(Expr::Bool(true)),
            TokenKind::Keyword(Keyword::False) => Ok(Expr::Bool(false)),
            TokenKind::Keyword(Keyword::Nil) => Ok(Expr::Nil),
            TokenKind::Identifier(name) => Ok(Expr::Variable {
                name,
                position: token.position,
            }),
            TokenKind::LeftParen => {
                let expression = self.expression()?;
                self.expect(TokenKind::RightParen)?;
                Ok(expression)
            }
            TokenKind::LeftBrace => self.dict_literal(),
            TokenKind::LeftBracket => Ok(Expr::List(self.list_items(TokenKind::RightBracket)?)),
            other => Err(Error::at(
                token.position,
                Message::ExpectedExpression(other.found()),
            )),
        }
    }

    /// `{"anahtar": değer, ...}`. Unlike blocks, a dictionary may span
    /// several lines freely.
    fn dict_literal(&mut self) -> Result<Expr, Error> {
        let mut entries = Vec::new();
        self.skip_newlines();
        while !self.check(&TokenKind::RightBrace) {
            let key_position = self.peek().position;
            let key = self.expression()?;
            self.skip_newlines();
            self.expect(TokenKind::Colon)?;
            self.skip_newlines();
            let value = self.expression()?;
            entries.push((key_position, key, value));
            self.skip_newlines();
            if !self.eat(&TokenKind::Comma) {
                break;
            }
            self.skip_newlines();
        }
        self.expect(TokenKind::RightBrace)?;
        Ok(Expr::Dict(entries))
    }

    fn skip_newlines(&mut self) {
        while self.check(&TokenKind::Newline) {
            self.advance();
        }
    }

    /// Comma-separated expressions up to `closing`; a trailing comma is allowed.
    fn list_items(&mut self, closing: TokenKind) -> Result<Vec<Expr>, Error> {
        let mut items = Vec::new();
        while !self.check(&closing) {
            items.push(self.expression()?);
            if !self.eat(&TokenKind::Comma) {
                break;
            }
        }
        self.expect(closing)?;
        Ok(items)
    }

    // ----- token helpers -----

    fn identifier(&mut self, expected: Expected) -> Result<String, Error> {
        let token = self.advance().clone();
        match token.kind {
            TokenKind::Identifier(name) => Ok(name),
            other => Err(Error::at(
                token.position,
                Message::Expected {
                    expected,
                    found: other.found(),
                },
            )),
        }
    }

    fn expect(&mut self, kind: TokenKind) -> Result<(), Error> {
        if self.eat(&kind) {
            return Ok(());
        }
        let token = self.peek();
        Err(Error::at(
            token.position,
            Message::Expected {
                expected: Expected::Token(kind.found()),
                found: token.kind.found(),
            },
        ))
    }

    fn eat(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    fn check(&self, kind: &TokenKind) -> bool {
        &self.peek().kind == kind
    }

    fn peek(&self) -> &Token {
        // The lexer always ends the stream with `Eof`, which is never consumed.
        &self.tokens[self.position.min(self.tokens.len() - 1)]
    }

    fn previous_kind(&self) -> Option<&TokenKind> {
        self.position
            .checked_sub(1)
            .and_then(|index| self.tokens.get(index))
            .map(|token| &token.kind)
    }

    fn advance(&mut self) -> &Token {
        let index = self.position.min(self.tokens.len() - 1);
        if self.tokens[index].kind != TokenKind::Eof {
            self.position += 1;
        }
        &self.tokens[index]
    }
}

#[cfg(test)]
mod tests {
    use super::parse;
    use crate::ast::{BinaryOp, Expr, Stmt};
    use crate::error::{Error, Position};
    use crate::keywords::Lang;
    use crate::lexer::lex;

    fn parse_source(source: &str) -> Result<Vec<Stmt>, Error> {
        parse(&lex(source)?).map(|program| program.statements)
    }

    #[test]
    fn respects_operator_precedence() {
        let statements = parse_source("1 + 2 * 3").unwrap();
        let Stmt::Expression(Expr::Binary { op, right, .. }) = &statements[0] else {
            panic!("expected a binary expression");
        };
        assert_eq!(*op, BinaryOp::Add);
        assert!(matches!(
            **right,
            Expr::Binary {
                op: BinaryOp::Multiply,
                ..
            }
        ));
    }

    #[test]
    fn parses_else_on_the_next_line() {
        let statements = parse_source("eğer doğru {\n}\ndeğilse {\n}").unwrap();
        assert!(matches!(
            statements[0],
            Stmt::If {
                else_branch: Some(_),
                ..
            }
        ));
    }

    #[test]
    fn rejects_missing_parenthesis_with_position() {
        let error = parse_source("yaz(\"Merhaba\"\nyaz(1)").unwrap_err();
        assert_eq!(error.position, Some(Position::new(2, 1)));
        assert_eq!(error.text(Lang::Tr), "')' bekleniyordu, 'yaz' bulundu");
    }

    #[test]
    fn rejects_two_statements_on_one_line() {
        let error = parse_source("yaz(1) yaz(2)").unwrap_err();
        assert_eq!(
            error.text(Lang::Tr),
            "Satır sonu bekleniyordu, 'yaz' bulundu"
        );
    }

    #[test]
    fn rejects_break_outside_loop() {
        let error = parse_source("kır").unwrap_err();
        assert_eq!(
            error.text(Lang::Tr),
            "'kır' yalnızca bir döngünün içinde kullanılabilir"
        );
    }

    #[test]
    fn rejects_break_in_function_inside_loop() {
        assert!(parse_source("iken doğru {\n fonksiyon f() { kır }\n}").is_err());
    }
}
