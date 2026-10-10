use std::rc::Rc;

use crate::error::Position;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Negate,
    Not,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Subtract,
    Multiply,
    Divide,
    Remainder,
    Equal,
    NotEqual,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogicalOp {
    And,
    Or,
}

#[derive(Debug, PartialEq)]
pub enum Expr {
    Number(f64),
    String(String),
    Bool(bool),
    Nil,
    List(Vec<Expr>),
    Variable {
        name: String,
        position: Position,
    },
    Unary {
        op: UnaryOp,
        operand: Box<Expr>,
        position: Position,
    },
    Binary {
        op: BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
        position: Position,
    },
    Logical {
        op: LogicalOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    Call {
        callee: Box<Expr>,
        arguments: Vec<Expr>,
        position: Position,
    },
    Index {
        target: Box<Expr>,
        index: Box<Expr>,
        position: Position,
    },
}

#[derive(Debug, PartialEq)]
pub struct FunctionDecl {
    pub name: String,
    pub parameters: Vec<String>,
    pub body: Vec<Stmt>,
}

#[derive(Debug, PartialEq)]
pub enum Stmt {
    Expression(Expr),
    Let {
        name: String,
        value: Expr,
    },
    Assign {
        name: String,
        value: Expr,
        position: Position,
    },
    IndexAssign {
        target: Expr,
        index: Expr,
        value: Expr,
        position: Position,
    },
    If {
        condition: Expr,
        then_branch: Vec<Stmt>,
        else_branch: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    For {
        variable: String,
        iterable: Expr,
        body: Vec<Stmt>,
        position: Position,
    },
    Function(Rc<FunctionDecl>),
    Return(Option<Expr>),
    Break,
    Continue,
}

#[derive(Debug, PartialEq)]
pub struct Program {
    pub statements: Vec<Stmt>,
}
