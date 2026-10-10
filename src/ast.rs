#[derive(Debug, PartialEq, Eq)]
pub struct Program {
    pub statements: Vec<Statement>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum Statement {
    Print(String),
}
