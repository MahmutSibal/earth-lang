use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::rc::Rc;

use crate::ast::FunctionDecl;
use crate::keywords::Builtin;

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    String(Rc<str>),
    List(Rc<RefCell<Vec<Value>>>),
    Function(Rc<Function>),
    Builtin(Builtin),
}

#[derive(Debug)]
pub struct Function {
    pub decl: Rc<FunctionDecl>,
    pub closure: Env,
}

impl Value {
    pub fn string(text: impl Into<Rc<str>>) -> Self {
        Value::String(text.into())
    }

    pub fn list(items: Vec<Value>) -> Self {
        Value::List(Rc::new(RefCell::new(items)))
    }

    /// Only `yanlış` and `boş` are false; everything else is true.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            Value::Nil => "boş",
            Value::Bool(_) => "mantıksal",
            Value::Number(_) => "sayı",
            Value::String(_) => "metin",
            Value::List(_) => "liste",
            Value::Function(_) | Value::Builtin(_) => "fonksiyon",
        }
    }

    /// The text shown inside lists, where strings keep their quotes.
    fn repr(&self) -> String {
        match self {
            Value::String(text) => format!("{text:?}"),
            other => other.to_string(),
        }
    }
}

impl PartialEq for Value {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Value::Nil, Value::Nil) => true,
            (Value::Bool(a), Value::Bool(b)) => a == b,
            (Value::Number(a), Value::Number(b)) => a == b,
            (Value::String(a), Value::String(b)) => a == b,
            (Value::List(a), Value::List(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (Value::Builtin(a), Value::Builtin(b)) => a == b,
            _ => false,
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Nil => write!(f, "boş"),
            Value::Bool(true) => write!(f, "doğru"),
            Value::Bool(false) => write!(f, "yanlış"),
            Value::Number(number) => write!(f, "{}", format_number(*number)),
            Value::String(text) => write!(f, "{text}"),
            Value::List(items) => {
                let items: Vec<String> = items.borrow().iter().map(Value::repr).collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Function(function) => write!(f, "<fonksiyon {}>", function.decl.name),
            Value::Builtin(builtin) => write!(f, "<fonksiyon {}>", builtin.name()),
        }
    }
}

/// Whole numbers print without a fractional part: `3`, not `3.0`.
pub fn format_number(number: f64) -> String {
    if number.fract() == 0.0 && number.abs() < 1e15 {
        format!("{}", number as i64)
    } else {
        format!("{number}")
    }
}

/// A lexical scope. Scopes are shared so that closures keep them alive.
pub type Env = Rc<RefCell<Scope>>;

#[derive(Debug, Default)]
pub struct Scope {
    values: HashMap<String, Value>,
    parent: Option<Env>,
}

impl Scope {
    pub fn new_child(parent: &Env) -> Env {
        Rc::new(RefCell::new(Scope {
            values: HashMap::new(),
            parent: Some(Rc::clone(parent)),
        }))
    }

    pub fn define(&mut self, name: impl Into<String>, value: Value) {
        self.values.insert(name.into(), value);
    }

    pub fn get(&self, name: &str) -> Option<Value> {
        match self.values.get(name) {
            Some(value) => Some(value.clone()),
            None => self.parent.as_ref()?.borrow().get(name),
        }
    }

    /// Updates an existing variable; returns `false` if it was never defined.
    pub fn assign(&mut self, name: &str, value: Value) -> bool {
        if let Some(slot) = self.values.get_mut(name) {
            *slot = value;
            return true;
        }
        match &self.parent {
            Some(parent) => parent.borrow_mut().assign(name, value),
            None => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{format_number, Value};

    #[test]
    fn formats_whole_numbers_without_decimals() {
        assert_eq!(format_number(3.0), "3");
        assert_eq!(format_number(-2.0), "-2");
        assert_eq!(format_number(0.5), "0.5");
    }

    #[test]
    fn displays_nested_lists_with_quoted_strings() {
        let value = Value::list(vec![
            Value::Number(1.0),
            Value::string("a"),
            Value::list(vec![Value::Bool(true)]),
        ]);
        assert_eq!(value.to_string(), "[1, \"a\", [doğru]]");
    }
}
