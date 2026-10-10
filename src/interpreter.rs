use std::io::{BufRead, Write};
use std::rc::Rc;

use crate::ast::{BinaryOp, Expr, LogicalOp, Program, Stmt, UnaryOp};
use crate::error::{Error, Position};
use crate::keywords::{builtins, Builtin, Lang};
use crate::messages::{Callee, Message};
use crate::stdlib::{self, dict_key, dict_keys, list_index};
use crate::value::{Dict, Env, Function, Scope, Value};

/// Deep enough for ordinary recursion, shallow enough to report an error
/// instead of overflowing the native stack (see `STACK_SIZE` in `main.rs`).
pub const MAX_CALL_DEPTH: usize = 1000;

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
    /// The language values are printed in: `doğru` or `true`.
    lang: Lang,
}

impl<'io> Interpreter<'io> {
    pub fn new(output: &'io mut dyn Write, input: &'io mut dyn BufRead, lang: Lang) -> Self {
        let globals = Env::default();
        for (name, builtin) in builtins() {
            globals.borrow_mut().define(name, Value::Builtin(builtin));
        }
        Self {
            globals,
            output,
            input,
            depth: 0,
            lang,
        }
    }

    pub fn lang(&self) -> Lang {
        self.lang
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
                    return Err(Error::at(
                        *position,
                        Message::UndefinedVariable(name.clone()),
                    ));
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
                            Message::NotAssignable(other.value_type()),
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
                            Message::NotIterable(other.value_type()),
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
            Stmt::Try {
                body,
                variable,
                handler,
            } => {
                let error = match self.execute_block(body, &Scope::new_child(env)) {
                    Ok(flow) => return Ok(flow),
                    Err(error) => error,
                };
                let scope = Scope::new_child(env);
                if let Some(variable) = variable {
                    // A thrown value is caught as is; Earth's own errors
                    // become their message, in the program's language.
                    let caught = match error.message {
                        Message::Thrown(value) => value,
                        message => Value::string(message.render(self.lang)),
                    };
                    scope.borrow_mut().define(variable.clone(), caught);
                }
                return self.execute_block(handler, &scope);
            }
            Stmt::Throw { value, position } => {
                let value = self.evaluate(value, env)?;
                return Err(Error::at(*position, Message::Thrown(value)));
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
                .ok_or_else(|| Error::at(*position, Message::UndefinedVariable(name.clone()))),
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
                        Message::InvalidOperand {
                            op: "-",
                            operand: operand.value_type(),
                        },
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
                self.binary(*op, left, right, *position)
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
                            Error::at(*position, Message::MissingKey(key.to_string()))
                        })
                    }
                    other => Err(Error::at(
                        *position,
                        Message::NotIndexable(other.value_type()),
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
                    Message::NotCallable(other.value_type()),
                ))
            }
        };

        let decl = &function.decl;
        if arguments.len() != decl.parameters.len() {
            let count = decl.parameters.len();
            return Err(Error::at(
                position,
                Message::WrongArity {
                    callee: Callee::Function(decl.name.clone()),
                    expected: (count, count),
                    given: arguments.len(),
                },
            ));
        }
        if self.depth >= MAX_CALL_DEPTH {
            return Err(Error::at(position, Message::RecursionLimit(MAX_CALL_DEPTH)));
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
        let (min, max) = stdlib::arity(builtin);
        if arguments.len() < min || arguments.len() > max {
            return Err(Error::at(
                position,
                Message::WrongArity {
                    callee: Callee::Builtin(builtin),
                    expected: (min, max),
                    given: arguments.len(),
                },
            ));
        }
        let io_error = |error: std::io::Error| Error::at(position, Message::Io(error.to_string()));
        let lang = self.lang;

        match builtin {
            Builtin::Print => {
                let line: Vec<String> = arguments
                    .iter()
                    .map(|argument| argument.display(lang).to_string())
                    .collect();
                writeln!(self.output, "{}", line.join(" ")).map_err(io_error)?;
                Ok(Value::Nil)
            }
            Builtin::Input => {
                if let Some(prompt) = arguments.first() {
                    write!(self.output, "{}", prompt.display(lang)).map_err(io_error)?;
                }
                self.output.flush().map_err(io_error)?;
                let mut line = String::new();
                if self.input.read_line(&mut line).map_err(io_error)? == 0 {
                    return Ok(Value::Nil);
                }
                let line = line.trim_end_matches(['\n', '\r']);
                Ok(Value::string(line))
            }
            _ => stdlib::call(builtin, &arguments, position, lang),
        }
    }

    fn binary(
        &self,
        op: BinaryOp,
        left: Value,
        right: Value,
        position: Position,
    ) -> Result<Value, Error> {
        use Value::{List, Number, String};

        let result = match (op, &left, &right) {
            (BinaryOp::Equal, _, _) => Value::Bool(left == right),
            (BinaryOp::NotEqual, _, _) => Value::Bool(left != right),

            (BinaryOp::Add, Number(a), Number(b)) => Number(a + b),
            (BinaryOp::Add, String(_), _) | (BinaryOp::Add, _, String(_)) => Value::string(
                format!("{}{}", left.display(self.lang), right.display(self.lang)),
            ),
            (BinaryOp::Add, List(a), List(b)) => {
                let mut items = a.borrow().clone();
                items.extend(b.borrow().iter().cloned());
                Value::list(items)
            }
            (BinaryOp::Subtract, Number(a), Number(b)) => Number(a - b),
            (BinaryOp::Multiply, Number(a), Number(b)) => Number(a * b),
            (BinaryOp::Divide | BinaryOp::Remainder, Number(_), Number(b)) if *b == 0.0 => {
                return Err(Error::at(position, Message::DivisionByZero));
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
                    Message::InvalidOperands {
                        op: symbol(op),
                        left: left.value_type(),
                        right: right.value_type(),
                    },
                ))
            }
        };
        Ok(result)
    }
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
