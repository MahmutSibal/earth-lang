use std::io::{BufRead, Write};
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, LogicalOp, Program, Stmt, UnaryOp};
use crate::error::{Error, Position};
use crate::keywords::{builtins, Builtin};
use crate::value::{format_number, Dict, Env, Function, Scope, Value};

/// Deep enough for ordinary recursion, shallow enough to report an error
/// instead of overflowing the native stack (see `STACK_SIZE` in `main.rs`).
pub const MAX_CALL_DEPTH: usize = 1000;

/// Upper bound on `aralık` results so that a typo cannot exhaust memory.
const MAX_RANGE_LENGTH: usize = 10_000_000;

/// How a statement finished: normally, or by jumping out of a block.
enum Flow {
    Normal,
    Break,
    Continue,
    Return(Value),
}

/// A tree-walking interpreter. Globals persist between calls to [`run`],
/// which lets the REPL build a program line by line.
///
/// [`run`]: Interpreter::run
pub struct Interpreter<'io> {
    globals: Env,
    output: &'io mut dyn Write,
    input: &'io mut dyn BufRead,
    depth: usize,
}

impl<'io> Interpreter<'io> {
    pub fn new(output: &'io mut dyn Write, input: &'io mut dyn BufRead) -> Self {
        let globals = Env::default();
        for (name, builtin) in builtins() {
            globals.borrow_mut().define(name, Value::Builtin(builtin));
        }
        Self {
            globals,
            output,
            input,
            depth: 0,
        }
    }

    /// Runs a program and returns the value of its last statement if that
    /// statement was an expression.
    pub fn run(&mut self, program: &Program) -> Result<Option<Value>, Error> {
        let globals = Rc::clone(&self.globals);
        let mut last = None;
        for statement in &program.statements {
            last = None;
            if let Stmt::Expression(expression) = statement {
                last = Some(self.evaluate(expression, &globals)?);
                continue;
            }
            // The parser rejects `döndür`, `kır` and `devam` at the top level.
            self.execute(statement, &globals)?;
        }
        Ok(last)
    }

    // ----- statements -----

    fn execute_block(&mut self, statements: &[Stmt], env: &Env) -> Result<Flow, Error> {
        for statement in statements {
            match self.execute(statement, env)? {
                Flow::Normal => {}
                flow => return Ok(flow),
            }
        }
        Ok(Flow::Normal)
    }

    fn execute(&mut self, statement: &Stmt, env: &Env) -> Result<Flow, Error> {
        match statement {
            Stmt::Expression(expression) => {
                self.evaluate(expression, env)?;
            }
            Stmt::Let { name, value } => {
                let value = self.evaluate(value, env)?;
                env.borrow_mut().define(name.clone(), value);
            }
            Stmt::Assign {
                name,
                value,
                position,
            } => {
                let value = self.evaluate(value, env)?;
                if !env.borrow_mut().assign(name, value) {
                    return Err(undefined_variable(name, *position));
                }
            }
            Stmt::IndexAssign {
                target,
                index,
                value,
                position,
            } => {
                let target = self.evaluate(target, env)?;
                let index = self.evaluate(index, env)?;
                let value = self.evaluate(value, env)?;
                match target {
                    Value::List(items) => {
                        let mut items = items.borrow_mut();
                        let slot = list_index(&index, items.len(), *position)?;
                        items[slot] = value;
                    }
                    Value::Dict(dict) => {
                        let key = dict_key(&index, *position)?;
                        dict.borrow_mut().insert(key, value);
                    }
                    other => {
                        return Err(Error::at(
                            *position,
                            format!(
                            "Yalnızca liste ve sözlük elemanlarına atama yapılabilir, {} bulundu",
                            other.type_name()
                        ),
                        ))
                    }
                }
            }
            Stmt::If {
                condition,
                then_branch,
                else_branch,
            } => {
                if self.evaluate(condition, env)?.is_truthy() {
                    return self.execute_block(then_branch, &Scope::new_child(env));
                }
                if let Some(else_branch) = else_branch {
                    return self.execute_block(else_branch, &Scope::new_child(env));
                }
            }
            Stmt::While { condition, body } => {
                while self.evaluate(condition, env)?.is_truthy() {
                    match self.execute_block(body, &Scope::new_child(env))? {
                        Flow::Break => break,
                        Flow::Normal | Flow::Continue => {}
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
            }
            Stmt::For {
                variable,
                iterable,
                body,
                position,
            } => {
                let items = match self.evaluate(iterable, env)? {
                    Value::List(items) => items.borrow().clone(),
                    Value::Dict(dict) => dict_keys(&dict.borrow()),
                    Value::String(text) => {
                        text.chars().map(|c| Value::string(c.to_string())).collect()
                    }
                    other => {
                        return Err(Error::at(
                            *position,
                            format!(
                                "'{}' türündeki bir değer üzerinde döngü kurulamaz",
                                other.type_name()
                            ),
                        ))
                    }
                };
                for item in items {
                    let scope = Scope::new_child(env);
                    scope.borrow_mut().define(variable.clone(), item);
                    match self.execute_block(body, &scope)? {
                        Flow::Break => break,
                        Flow::Normal | Flow::Continue => {}
                        flow @ Flow::Return(_) => return Ok(flow),
                    }
                }
            }
            Stmt::Function(decl) => {
                let function = Function {
                    decl: Rc::clone(decl),
                    closure: Rc::clone(env),
                };
                env.borrow_mut()
                    .define(decl.name.clone(), Value::Function(Rc::new(function)));
            }
            Stmt::Return(value) => {
                let value = match value {
                    Some(value) => self.evaluate(value, env)?,
                    None => Value::Nil,
                };
                return Ok(Flow::Return(value));
            }
            Stmt::Break => return Ok(Flow::Break),
            Stmt::Continue => return Ok(Flow::Continue),
        }
        Ok(Flow::Normal)
    }

    // ----- expressions -----

    fn evaluate(&mut self, expression: &Expr, env: &Env) -> Result<Value, Error> {
        match expression {
            Expr::Number(value) => Ok(Value::Number(*value)),
            Expr::String(value) => Ok(Value::string(value.as_str())),
            Expr::Bool(value) => Ok(Value::Bool(*value)),
            Expr::Nil => Ok(Value::Nil),
            Expr::List(items) => {
                let items = items
                    .iter()
                    .map(|item| self.evaluate(item, env))
                    .collect::<Result<_, _>>()?;
                Ok(Value::list(items))
            }
            Expr::Dict(entries) => {
                let mut dict = Dict::default();
                for (key_position, key, value) in entries {
                    let key = dict_key(&self.evaluate(key, env)?, *key_position)?;
                    let value = self.evaluate(value, env)?;
                    dict.insert(key, value);
                }
                Ok(Value::dict(dict))
            }
            Expr::Variable { name, position } => env
                .borrow()
                .get(name)
                .ok_or_else(|| undefined_variable(name, *position)),
            Expr::Unary {
                op,
                operand,
                position,
            } => {
                let operand = self.evaluate(operand, env)?;
                match (op, operand) {
                    (UnaryOp::Not, operand) => Ok(Value::Bool(!operand.is_truthy())),
                    (UnaryOp::Negate, Value::Number(number)) => Ok(Value::Number(-number)),
                    (UnaryOp::Negate, operand) => Err(Error::at(
                        *position,
                        format!("'-' işlemi '{}' türüne uygulanamaz", operand.type_name()),
                    )),
                }
            }
            Expr::Binary {
                op,
                left,
                right,
                position,
            } => {
                let left = self.evaluate(left, env)?;
                let right = self.evaluate(right, env)?;
                binary(*op, left, right, *position)
            }
            Expr::Logical { op, left, right } => {
                let left = self.evaluate(left, env)?;
                let short_circuits = match op {
                    LogicalOp::And => !left.is_truthy(),
                    LogicalOp::Or => left.is_truthy(),
                };
                if short_circuits {
                    Ok(left)
                } else {
                    self.evaluate(right, env)
                }
            }
            Expr::Call {
                callee,
                arguments,
                position,
            } => {
                let callee = self.evaluate(callee, env)?;
                let arguments = arguments
                    .iter()
                    .map(|argument| self.evaluate(argument, env))
                    .collect::<Result<Vec<_>, _>>()?;
                self.call(callee, arguments, *position)
            }
            Expr::Index {
                target,
                index,
                position,
            } => {
                let target = self.evaluate(target, env)?;
                let index = self.evaluate(index, env)?;
                match target {
                    Value::List(items) => {
                        let items = items.borrow();
                        Ok(items[list_index(&index, items.len(), *position)?].clone())
                    }
                    Value::String(text) => {
                        let chars: Vec<char> = text.chars().collect();
                        let slot = list_index(&index, chars.len(), *position)?;
                        Ok(Value::string(chars[slot].to_string()))
                    }
                    Value::Dict(dict) => {
                        let key = dict_key(&index, *position)?;
                        let dict = dict.borrow();
                        dict.get(&key).cloned().ok_or_else(|| {
                            Error::at(
                                *position,
                                format!(
                                    "Sözlükte {key:?} anahtarı yok ('içerir' ile kontrol edebilirsiniz)"
                                ),
                            )
                        })
                    }
                    other => Err(Error::at(
                        *position,
                        format!("'{}' türündeki bir değer indekslenemez", other.type_name()),
                    )),
                }
            }
        }
    }

    fn call(
        &mut self,
        callee: Value,
        arguments: Vec<Value>,
        position: Position,
    ) -> Result<Value, Error> {
        let function = match callee {
            Value::Function(function) => function,
            Value::Builtin(builtin) => return self.call_builtin(builtin, arguments, position),
            other => {
                return Err(Error::at(
                    position,
                    format!("'{}' türündeki bir değer çağrılamaz", other.type_name()),
                ))
            }
        };

        let decl = &function.decl;
        if arguments.len() != decl.parameters.len() {
            return Err(Error::at(
                position,
                format!(
                    "'{}' {} argüman bekliyor, {} verildi",
                    decl.name,
                    decl.parameters.len(),
                    arguments.len()
                ),
            ));
        }
        if self.depth >= MAX_CALL_DEPTH {
            return Err(Error::at(
                position,
                format!("Çok derin özyineleme (en fazla {MAX_CALL_DEPTH} iç içe çağrı)"),
            ));
        }

        let scope = Scope::new_child(&function.closure);
        for (parameter, argument) in decl.parameters.iter().zip(arguments) {
            scope.borrow_mut().define(parameter.clone(), argument);
        }

        self.depth += 1;
        let result = self.execute_block(&decl.body, &scope);
        self.depth -= 1;

        match result? {
            Flow::Return(value) => Ok(value),
            _ => Ok(Value::Nil),
        }
    }

    fn call_builtin(
        &mut self,
        builtin: Builtin,
        arguments: Vec<Value>,
        position: Position,
    ) -> Result<Value, Error> {
        let (min, max) = match builtin {
            Builtin::Print => (0, usize::MAX),
            Builtin::Input => (0, 1),
            Builtin::Length
            | Builtin::ToText
            | Builtin::ToNumber
            | Builtin::Type
            | Builtin::Keys
            | Builtin::Values => (1, 1),
            Builtin::Range => (1, 3),
            Builtin::Push | Builtin::Contains | Builtin::Remove => (2, 2),
        };
        if arguments.len() < min || arguments.len() > max {
            let expected = if min == max {
                min.to_string()
            } else {
                format!("{min}-{max}")
            };
            return Err(Error::at(
                position,
                format!(
                    "'{}' {expected} argüman bekliyor, {} verildi",
                    builtin.name(),
                    arguments.len()
                ),
            ));
        }
        let io_error = |error: std::io::Error| Error::at(position, format!("G/Ç hatası: {error}"));

        match builtin {
            Builtin::Print => {
                let line: Vec<String> = arguments.iter().map(Value::to_string).collect();
                writeln!(self.output, "{}", line.join(" ")).map_err(io_error)?;
                Ok(Value::Nil)
            }
            Builtin::Input => {
                if let Some(prompt) = arguments.first() {
                    write!(self.output, "{prompt}").map_err(io_error)?;
                }
                self.output.flush().map_err(io_error)?;
                let mut line = String::new();
                if self.input.read_line(&mut line).map_err(io_error)? == 0 {
                    return Ok(Value::Nil);
                }
                let line = line.trim_end_matches(['\n', '\r']);
                Ok(Value::string(line))
            }
            Builtin::Length => match &arguments[0] {
                Value::String(text) => Ok(Value::Number(text.chars().count() as f64)),
                Value::List(items) => Ok(Value::Number(items.borrow().len() as f64)),
                Value::Dict(dict) => Ok(Value::Number(dict.borrow().len() as f64)),
                other => Err(Error::at(
                    position,
                    format!("'{}' türünün uzunluğu yoktur", other.type_name()),
                )),
            },
            Builtin::Range => range(&arguments, position),
            Builtin::ToText => Ok(Value::string(arguments[0].to_string())),
            Builtin::ToNumber => Ok(match &arguments[0] {
                Value::Number(number) => Value::Number(*number),
                Value::String(text) => match text.trim().parse::<f64>() {
                    Ok(number) if number.is_finite() => Value::Number(number),
                    _ => Value::Nil,
                },
                _ => Value::Nil,
            }),
            Builtin::Push => match &arguments[0] {
                Value::List(items) => {
                    items.borrow_mut().push(arguments[1].clone());
                    Ok(Value::Nil)
                }
                other => Err(Error::at(
                    position,
                    format!(
                        "'ekle' ilk argüman olarak liste bekliyor, {} verildi",
                        other.type_name()
                    ),
                )),
            },
            Builtin::Type => Ok(Value::string(arguments[0].type_name())),
            Builtin::Keys => match &arguments[0] {
                Value::Dict(dict) => Ok(Value::list(dict_keys(&dict.borrow()))),
                other => Err(expected_dict(builtin, other, position)),
            },
            Builtin::Values => match &arguments[0] {
                Value::Dict(dict) => Ok(Value::list(
                    dict.borrow()
                        .iter()
                        .map(|(_, value)| value.clone())
                        .collect(),
                )),
                other => Err(expected_dict(builtin, other, position)),
            },
            Builtin::Contains => {
                let needle = &arguments[1];
                let found = match &arguments[0] {
                    Value::List(items) => items.borrow().contains(needle),
                    Value::Dict(dict) => match needle {
                        Value::String(key) => dict.borrow().contains(key),
                        _ => false,
                    },
                    Value::String(text) => match needle {
                        Value::String(part) => text.contains(&**part),
                        other => {
                            return Err(Error::at(
                                position,
                                format!(
                                    "Metin içinde yalnızca metin aranabilir, {} verildi",
                                    other.type_name()
                                ),
                            ))
                        }
                    },
                    other => {
                        return Err(Error::at(
                            position,
                            format!(
                                "'içerir' liste, sözlük veya metin bekliyor, {} verildi",
                                other.type_name()
                            ),
                        ))
                    }
                };
                Ok(Value::Bool(found))
            }
            Builtin::Remove => match &arguments[0] {
                Value::Dict(dict) => {
                    let key = dict_key(&arguments[1], position)?;
                    Ok(dict.borrow_mut().remove(&key).unwrap_or(Value::Nil))
                }
                Value::List(items) => {
                    let mut items = items.borrow_mut();
                    let slot = list_index(&arguments[1], items.len(), position)?;
                    Ok(items.remove(slot))
                }
                other => Err(Error::at(
                    position,
                    format!(
                        "'sil' liste veya sözlük bekliyor, {} verildi",
                        other.type_name()
                    ),
                )),
            },
        }
    }
}

/// Dictionary keys are text; anything else is reported at `position`.
fn dict_key(key: &Value, position: Position) -> Result<Rc<str>, Error> {
    match key {
        Value::String(text) => Ok(Rc::clone(text)),
        other => Err(Error::at(
            position,
            format!(
                "Sözlük anahtarı metin olmalı, {} bulundu (gerekirse metin(...) kullanın)",
                other.type_name()
            ),
        )),
    }
}

fn dict_keys(dict: &Dict) -> Vec<Value> {
    dict.iter()
        .map(|(key, _)| Value::String(Rc::clone(key)))
        .collect()
}

fn expected_dict(builtin: Builtin, found: &Value, position: Position) -> Error {
    Error::at(
        position,
        format!(
            "'{}' sözlük bekliyor, {} verildi",
            builtin.name(),
            found.type_name()
        ),
    )
}

fn undefined_variable(name: &str, position: Position) -> Error {
    Error::at(
        position,
        format!("Tanımsız değişken '{name}' (önce 'değişken {name} = ...' ile tanımlayın)"),
    )
}

/// Converts an index value into a slot, counting negative indexes from the end.
fn list_index(index: &Value, length: usize, position: Position) -> Result<usize, Error> {
    let Value::Number(number) = *index else {
        return Err(Error::at(
            position,
            format!("İndeks sayı olmalı, {} bulundu", index.type_name()),
        ));
    };
    if number.fract() != 0.0 {
        return Err(Error::at(
            position,
            format!("İndeks tam sayı olmalı, {} bulundu", format_number(number)),
        ));
    }
    let resolved = if number < 0.0 {
        length as f64 + number
    } else {
        number
    };
    if resolved < 0.0 || resolved >= length as f64 {
        return Err(Error::at(
            position,
            format!(
                "İndeks {} sınırların dışında (uzunluk {length})",
                format_number(number)
            ),
        ));
    }
    Ok(resolved as usize)
}

fn range(arguments: &[Value], position: Position) -> Result<Value, Error> {
    let mut numbers = Vec::with_capacity(arguments.len());
    for argument in arguments {
        match argument {
            Value::Number(number) => numbers.push(*number),
            other => {
                return Err(Error::at(
                    position,
                    format!("'aralık' sayı bekliyor, {} verildi", other.type_name()),
                ))
            }
        }
    }
    let (start, end, step) = match numbers[..] {
        [end] => (0.0, end, 1.0),
        [start, end] => (start, end, 1.0),
        [start, end, step] => (start, end, step),
        _ => unreachable!("arity is checked by the caller"),
    };
    if step == 0.0 || !step.is_finite() {
        return Err(Error::at(position, "'aralık' adımı sıfır olamaz"));
    }

    let length = ((end - start) / step).ceil().max(0.0);
    if !length.is_finite() || length > MAX_RANGE_LENGTH as f64 {
        return Err(Error::at(
            position,
            format!("'aralık' en fazla {MAX_RANGE_LENGTH} eleman üretebilir"),
        ));
    }
    let items = (0..length as usize)
        .map(|i| Value::Number(start + step * i as f64))
        .collect();
    Ok(Value::list(items))
}

fn binary(op: BinaryOp, left: Value, right: Value, position: Position) -> Result<Value, Error> {
    use Value::{List, Number, String};

    let result = match (op, &left, &right) {
        (BinaryOp::Equal, _, _) => Value::Bool(left == right),
        (BinaryOp::NotEqual, _, _) => Value::Bool(left != right),

        (BinaryOp::Add, Number(a), Number(b)) => Number(a + b),
        (BinaryOp::Add, String(_), _) | (BinaryOp::Add, _, String(_)) => {
            Value::string(format!("{left}{right}"))
        }
        (BinaryOp::Add, List(a), List(b)) => {
            let mut items = a.borrow().clone();
            items.extend(b.borrow().iter().cloned());
            Value::list(items)
        }
        (BinaryOp::Subtract, Number(a), Number(b)) => Number(a - b),
        (BinaryOp::Multiply, Number(a), Number(b)) => Number(a * b),
        (BinaryOp::Divide | BinaryOp::Remainder, Number(_), Number(b)) if *b == 0.0 => {
            return Err(Error::at(position, "Sıfıra bölme"));
        }
        (BinaryOp::Divide, Number(a), Number(b)) => Number(a / b),
        (BinaryOp::Remainder, Number(a), Number(b)) => Number(a % b),

        (BinaryOp::Less, Number(a), Number(b)) => Value::Bool(a < b),
        (BinaryOp::LessEqual, Number(a), Number(b)) => Value::Bool(a <= b),
        (BinaryOp::Greater, Number(a), Number(b)) => Value::Bool(a > b),
        (BinaryOp::GreaterEqual, Number(a), Number(b)) => Value::Bool(a >= b),
        (BinaryOp::Less, String(a), String(b)) => Value::Bool(a < b),
        (BinaryOp::LessEqual, String(a), String(b)) => Value::Bool(a <= b),
        (BinaryOp::Greater, String(a), String(b)) => Value::Bool(a > b),
        (BinaryOp::GreaterEqual, String(a), String(b)) => Value::Bool(a >= b),

        _ => {
            return Err(Error::at(
                position,
                format!(
                    "'{}' işlemi '{}' ve '{}' türleri arasında yapılamaz",
                    symbol(op),
                    left.type_name(),
                    right.type_name()
                ),
            ))
        }
    };
    Ok(result)
}

fn symbol(op: BinaryOp) -> &'static str {
    match op {
        BinaryOp::Add => "+",
        BinaryOp::Subtract => "-",
        BinaryOp::Multiply => "*",
        BinaryOp::Divide => "/",
        BinaryOp::Remainder => "%",
        BinaryOp::Equal => "==",
        BinaryOp::NotEqual => "!=",
        BinaryOp::Less => "<",
        BinaryOp::LessEqual => "<=",
        BinaryOp::Greater => ">",
        BinaryOp::GreaterEqual => ">=",
    }
}
