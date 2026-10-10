use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt;
use std::path::Path;
use std::rc::Rc;

use crate::ast::FunctionDecl;
use crate::keywords::{Builtin, Keyword, Lang};
use crate::messages::Type;

#[derive(Debug, Clone)]
pub enum Value {
    Nil,
    Bool(bool),
    Number(f64),
    String(Rc<str>),
    List(Rc<RefCell<Vec<Value>>>),
    Dict(Rc<RefCell<Dict>>),
    Function(Rc<Function>),
    Builtin(Builtin),
}

#[derive(Debug)]
pub struct Function {
    pub decl: Rc<FunctionDecl>,
    pub closure: Env,
    /// The imported file the function was defined in, for error reports.
    pub file: Option<Rc<Path>>,
}

impl Value {
    pub fn string(text: impl Into<Rc<str>>) -> Self {
        Value::String(text.into())
    }

    pub fn list(items: Vec<Value>) -> Self {
        Value::List(Rc::new(RefCell::new(items)))
    }

    pub fn dict(dict: Dict) -> Self {
        Value::Dict(Rc::new(RefCell::new(dict)))
    }

    /// Only `yanlış` and `boş` are false; everything else is true.
    pub fn is_truthy(&self) -> bool {
        !matches!(self, Value::Nil | Value::Bool(false))
    }

    pub fn value_type(&self) -> Type {
        match self {
            Value::Nil => Type::Nil,
            Value::Bool(_) => Type::Bool,
            Value::Number(_) => Type::Number,
            Value::String(_) => Type::Text,
            Value::List(_) => Type::List,
            Value::Dict(_) => Type::Dict,
            Value::Function(_) | Value::Builtin(_) => Type::Function,
        }
    }

    /// The value as `yaz` prints it, in `lang`: `doğru` or `true`.
    pub fn display(&self, lang: Lang) -> Displayed<'_> {
        Displayed { value: self, lang }
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
            (Value::Dict(a), Value::Dict(b)) => Rc::ptr_eq(a, b) || *a.borrow() == *b.borrow(),
            (Value::Function(a), Value::Function(b)) => Rc::ptr_eq(a, b),
            (Value::Builtin(a), Value::Builtin(b)) => a == b,
            _ => false,
        }
    }
}

/// A value paired with the language it is shown in.
pub struct Displayed<'a> {
    value: &'a Value,
    lang: Lang,
}

impl Displayed<'_> {
    /// Inside lists and dictionaries strings keep their quotes.
    fn repr(&self, value: &Value) -> String {
        match value {
            Value::String(text) => format!("{text:?}"),
            other => other.display(self.lang).to_string(),
        }
    }
}

impl fmt::Display for Displayed<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let lang = self.lang;
        match self.value {
            Value::Nil => write!(f, "{}", Keyword::Nil.name(lang)),
            Value::Bool(true) => write!(f, "{}", Keyword::True.name(lang)),
            Value::Bool(false) => write!(f, "{}", Keyword::False.name(lang)),
            Value::Number(number) => write!(f, "{}", format_number(*number)),
            Value::String(text) => write!(f, "{text}"),
            Value::List(items) => {
                let Some(_guard) = CycleGuard::enter(Rc::as_ptr(items).cast()) else {
                    return write!(f, "[...]");
                };
                let items: Vec<String> = items.borrow().iter().map(|v| self.repr(v)).collect();
                write!(f, "[{}]", items.join(", "))
            }
            Value::Dict(dict) => {
                let Some(_guard) = CycleGuard::enter(Rc::as_ptr(dict).cast()) else {
                    return write!(f, "{{...}}");
                };
                let entries: Vec<String> = dict
                    .borrow()
                    .iter()
                    .map(|(key, value)| format!("{key:?}: {}", self.repr(value)))
                    .collect();
                write!(f, "{{{}}}", entries.join(", "))
            }
            Value::Function(function) => {
                write!(f, "<{} {}>", Type::Function.name(lang), function.decl.name)
            }
            Value::Builtin(builtin) => {
                write!(f, "<{} {}>", Type::Function.name(lang), builtin.name(lang))
            }
        }
    }
}

/// Shows the value in Turkish; use [`Value::display`] for other languages.
impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.display(Lang::Tr).fmt(f)
    }
}

thread_local! {
    /// Lists and dictionaries currently being printed, innermost last.
    static PRINTING: std::cell::RefCell<Vec<*const ()>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Marks a container as being printed so that a container which contains
/// itself prints as `[...]` instead of recursing forever.
struct CycleGuard;

impl CycleGuard {
    fn enter(container: *const ()) -> Option<Self> {
        PRINTING.with(|printing| {
            let mut printing = printing.borrow_mut();
            if printing.contains(&container) {
                return None;
            }
            printing.push(container);
            Some(CycleGuard)
        })
    }
}

impl Drop for CycleGuard {
    fn drop(&mut self) {
        PRINTING.with(|printing| {
            printing.borrow_mut().pop();
        });
    }
}

/// A dictionary with text keys that remembers insertion order, so that
/// printing and iterating are predictable.
#[derive(Debug, Default, Clone)]
pub struct Dict {
    entries: Vec<(Rc<str>, Value)>,
    positions: HashMap<Rc<str>, usize>,
}

impl Dict {
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.positions.get(key).map(|&slot| &self.entries[slot].1)
    }

    pub fn insert(&mut self, key: Rc<str>, value: Value) {
        match self.positions.get(&key) {
            Some(&slot) => self.entries[slot].1 = value,
            None => {
                self.positions.insert(Rc::clone(&key), self.entries.len());
                self.entries.push((key, value));
            }
        }
    }

    pub fn remove(&mut self, key: &str) -> Option<Value> {
        let slot = self.positions.remove(key)?;
        let (_, value) = self.entries.remove(slot);
        for position in self.positions.values_mut() {
            if *position > slot {
                *position -= 1;
            }
        }
        Some(value)
    }

    pub fn contains(&self, key: &str) -> bool {
        self.positions.contains_key(key)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (&Rc<str>, &Value)> {
        self.entries.iter().map(|(key, value)| (key, value))
    }
}

/// Two dictionaries are equal when they hold the same pairs, in any order.
impl PartialEq for Dict {
    fn eq(&self, other: &Self) -> bool {
        self.len() == other.len()
            && self
                .iter()
                .all(|(key, value)| other.get(key) == Some(value))
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

    /// The variables defined directly in this scope, not in its parents.
    pub fn own_values(&self) -> impl Iterator<Item = (&String, &Value)> {
        self.values.iter()
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
    use super::{format_number, Dict, Value};
    use crate::keywords::Lang;

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

    #[test]
    fn dictionaries_keep_insertion_order() {
        let mut dict = Dict::default();
        dict.insert("b".into(), Value::Number(1.0));
        dict.insert("a".into(), Value::Number(2.0));
        dict.insert("c".into(), Value::Number(3.0));
        dict.insert("b".into(), Value::Number(4.0));
        assert_eq!(dict.remove("a"), Some(Value::Number(2.0)));
        assert_eq!(dict.get("c"), Some(&Value::Number(3.0)));
        assert_eq!(Value::dict(dict).to_string(), "{\"b\": 4, \"c\": 3}");
    }

    #[test]
    fn prints_self_referencing_containers() {
        let list = Value::list(vec![Value::Number(1.0)]);
        if let Value::List(items) = &list {
            items.borrow_mut().push(list.clone());
        }
        assert_eq!(list.to_string(), "[1, [...]]");

        let dict = Value::dict(Dict::default());
        if let Value::Dict(entries) = &dict {
            entries.borrow_mut().insert("ben".into(), dict.clone());
        }
        assert_eq!(dict.to_string(), "{\"ben\": {...}}");
    }

    #[test]
    fn displays_values_in_each_language() {
        let value = Value::list(vec![Value::Bool(true), Value::Nil]);
        assert_eq!(value.display(Lang::Tr).to_string(), "[doğru, boş]");
        assert_eq!(value.display(Lang::En).to_string(), "[true, nil]");
    }
}
