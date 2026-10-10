use crate::ast::{Program, Statement};

pub fn interpret(program: &Program) -> Result<Vec<String>, String> {
    program
        .statements
        .iter()
        .map(|statement| match statement {
            Statement::Print(value) => Ok(value.clone()),
        })
        .collect()
}
