//! Built-in functions that only compute a value from their arguments.
//!
//! Built-ins that read or write (`yaz`, `girdi`) live in the interpreter,
//! which owns the input and output streams.

use std::cell::Cell;
use std::cmp::Ordering;
use std::rc::Rc;

use crate::error::{Error, Position};
use crate::keywords::{Builtin, Lang};
use crate::messages::{Message, Type};
use crate::value::{Dict, Value};

/// Upper bound on `aralık` results so that a typo cannot exhaust memory.
const MAX_RANGE_LENGTH: usize = 10_000_000;

/// The smallest and largest number of arguments a built-in accepts.
pub fn arity(builtin: Builtin) -> (usize, usize) {
    match builtin {
        Builtin::Print | Builtin::Min | Builtin::Max => (0, usize::MAX),
        Builtin::Input | Builtin::Random => (0, 2),
        Builtin::Length
        | Builtin::ToText
        | Builtin::ToNumber
        | Builtin::Type
        | Builtin::Keys
        | Builtin::Values
        | Builtin::Upper
        | Builtin::Lower
        | Builtin::Trim
        | Builtin::Abs
        | Builtin::Sqrt
        | Builtin::Floor
        | Builtin::Ceil
        | Builtin::Sum
        | Builtin::Sort
        | Builtin::Reverse => (1, 1),
        Builtin::Round | Builtin::Split | Builtin::Join => (1, 2),
        Builtin::Range | Builtin::Slice => (1, 3),
        Builtin::Push | Builtin::Contains | Builtin::Remove | Builtin::Power => (2, 2),
        Builtin::Replace => (3, 3),
    }
}

/// Runs a pure built-in. The caller has already checked the arity.
pub fn call(
    builtin: Builtin,
    arguments: &[Value],
    position: Position,
    lang: Lang,
) -> Result<Value, Error> {
    let wrong = |expected: &'static [Type], given: &Value| {
        Error::at(
            position,
            Message::WrongArgument {
                builtin,
                expected,
                given: given.value_type(),
            },
        )
    };
    let text = |index: usize| match &arguments[index] {
        Value::String(text) => Ok(Rc::clone(text)),
        other => Err(wrong(&[Type::Text], other)),
    };
    let number = |index: usize| match arguments[index] {
        Value::Number(number) => Ok(number),
        ref other => Err(wrong(&[Type::Number], other)),
    };
    let list = |index: usize| match &arguments[index] {
        Value::List(items) => Ok(items.borrow().clone()),
        other => Err(wrong(&[Type::List], other)),
    };

    let value = match builtin {
        Builtin::Print | Builtin::Input => unreachable!("handled by the interpreter"),

        Builtin::Length => match &arguments[0] {
            Value::String(text) => Value::Number(text.chars().count() as f64),
            Value::List(items) => Value::Number(items.borrow().len() as f64),
            Value::Dict(dict) => Value::Number(dict.borrow().len() as f64),
            other => return Err(Error::at(position, Message::NoLength(other.value_type()))),
        },
        Builtin::Range => range(arguments, position)?,
        Builtin::ToText => Value::string(arguments[0].display(lang).to_string()),
        Builtin::ToNumber => match &arguments[0] {
            Value::Number(number) => Value::Number(*number),
            Value::String(text) => match text.trim().parse::<f64>() {
                Ok(number) if number.is_finite() => Value::Number(number),
                _ => Value::Nil,
            },
            _ => Value::Nil,
        },
        Builtin::Type => Value::string(arguments[0].value_type().name(lang)),

        // Lists and dictionaries
        Builtin::Push => match &arguments[0] {
            Value::List(items) => {
                items.borrow_mut().push(arguments[1].clone());
                Value::Nil
            }
            other => return Err(wrong(&[Type::List], other)),
        },
        Builtin::Keys => match &arguments[0] {
            Value::Dict(dict) => Value::list(dict_keys(&dict.borrow())),
            other => return Err(wrong(&[Type::Dict], other)),
        },
        Builtin::Values => match &arguments[0] {
            Value::Dict(dict) => Value::list(
                dict.borrow()
                    .iter()
                    .map(|(_, value)| value.clone())
                    .collect(),
            ),
            other => return Err(wrong(&[Type::Dict], other)),
        },
        Builtin::Contains => {
            let needle = &arguments[1];
            let found = match &arguments[0] {
                Value::List(items) => items.borrow().contains(needle),
                Value::Dict(dict) => match needle {
                    Value::String(key) => dict.borrow().contains(key),
                    _ => false,
                },
                Value::String(haystack) => match needle {
                    Value::String(part) => haystack.contains(&**part),
                    other => return Err(wrong(&[Type::Text], other)),
                },
                other => return Err(wrong(&[Type::List, Type::Dict, Type::Text], other)),
            };
            Value::Bool(found)
        }
        Builtin::Remove => match &arguments[0] {
            Value::Dict(dict) => {
                let key = dict_key(&arguments[1], position)?;
                dict.borrow_mut().remove(&key).unwrap_or(Value::Nil)
            }
            Value::List(items) => {
                let mut items = items.borrow_mut();
                let slot = list_index(&arguments[1], items.len(), position)?;
                items.remove(slot)
            }
            other => return Err(wrong(&[Type::List, Type::Dict], other)),
        },
        Builtin::Sort => {
            let mut items = list(0)?;
            sort(&mut items, position)?;
            Value::list(items)
        }
        Builtin::Reverse => match &arguments[0] {
            Value::List(items) => Value::list(items.borrow().iter().rev().cloned().collect()),
            Value::String(text) => Value::string(text.chars().rev().collect::<String>()),
            other => return Err(wrong(&[Type::List, Type::Text], other)),
        },
        Builtin::Slice => slice(arguments, position, wrong)?,
        Builtin::Sum => {
            let mut total = 0.0;
            for item in list(0)? {
                match item {
                    Value::Number(number) => total += number,
                    other => return Err(wrong(&[Type::Number], &other)),
                }
            }
            Value::Number(total)
        }
        Builtin::Min | Builtin::Max => {
            // Either several numbers, or a single list of numbers.
            let items = match arguments {
                [Value::List(items)] => items.borrow().clone(),
                _ => arguments.to_vec(),
            };
            let mut best: Option<f64> = None;
            for item in &items {
                let Value::Number(number) = *item else {
                    return Err(wrong(&[Type::Number], item));
                };
                best = Some(match best {
                    None => number,
                    Some(best) if builtin == Builtin::Min => best.min(number),
                    Some(best) => best.max(number),
                });
            }
            match best {
                Some(best) => Value::Number(best),
                None => return Err(Error::at(position, Message::EmptySequence(builtin))),
            }
        }

        // Text
        Builtin::Upper => Value::string(upper(&text(0)?, lang)),
        Builtin::Lower => Value::string(lower(&text(0)?, lang)),
        Builtin::Trim => Value::string(text(0)?.trim()),
        Builtin::Split => {
            let whole = text(0)?;
            let parts: Vec<Value> = if arguments.len() == 1 {
                whole.split_whitespace().map(Value::string).collect()
            } else {
                let separator = text(1)?;
                if separator.is_empty() {
                    return Err(Error::at(position, Message::EmptySeparator));
                }
                whole.split(&*separator).map(Value::string).collect()
            };
            Value::list(parts)
        }
        Builtin::Join => {
            let separator = if arguments.len() == 2 {
                text(1)?.to_string()
            } else {
                String::new()
            };
            let parts: Vec<String> = list(0)?
                .iter()
                .map(|item| item.display(lang).to_string())
                .collect();
            Value::string(parts.join(&separator))
        }
        Builtin::Replace => {
            let (whole, from, to) = (text(0)?, text(1)?, text(2)?);
            if from.is_empty() {
                return Err(Error::at(position, Message::EmptySeparator));
            }
            Value::string(whole.replace(&*from, &to))
        }

        // Numbers
        Builtin::Abs => Value::Number(number(0)?.abs()),
        Builtin::Sqrt => Value::Number(number(0)?.sqrt()),
        Builtin::Power => Value::Number(number(0)?.powf(number(1)?)),
        Builtin::Floor => Value::Number(number(0)?.floor()),
        Builtin::Ceil => Value::Number(number(0)?.ceil()),
        Builtin::Round => {
            let value = number(0)?;
            let digits = if arguments.len() == 2 {
                number(1)?
            } else {
                0.0
            };
            let scale = 10f64.powf(digits.trunc());
            Value::Number((value * scale).round() / scale)
        }
        Builtin::Random => match arguments.len() {
            // A fraction in [0, 1).
            0 => Value::Number(random_fraction()),
            // A whole number in [0, n) or [a, b].
            1 => Value::Number((random_fraction() * number(0)?.floor()).floor()),
            _ => {
                let (low, high) = (number(0)?.ceil(), number(1)?.floor());
                if high < low {
                    return Err(Error::at(position, Message::EmptySequence(builtin)));
                }
                Value::Number(low + (random_fraction() * (high - low + 1.0)).floor())
            }
        },
    };
    Ok(value)
}

/// Dictionary keys are text; anything else is reported at `position`.
pub fn dict_key(key: &Value, position: Position) -> Result<Rc<str>, Error> {
    match key {
        Value::String(text) => Ok(Rc::clone(text)),
        other => Err(Error::at(position, Message::KeyNotText(other.value_type()))),
    }
}

pub fn dict_keys(dict: &Dict) -> Vec<Value> {
    dict.iter()
        .map(|(key, _)| Value::String(Rc::clone(key)))
        .collect()
}

/// Converts an index value into a slot, counting negative indexes from the end.
pub fn list_index(index: &Value, length: usize, position: Position) -> Result<usize, Error> {
    let slot = resolve_index(index, length, position)?;
    if slot < 0.0 || slot >= length as f64 {
        let Value::Number(number) = *index else {
            unreachable!("resolve_index only accepts numbers")
        };
        return Err(Error::at(
            position,
            Message::IndexOutOfRange {
                index: number,
                length,
            },
        ));
    }
    Ok(slot as usize)
}

/// A whole-number index with negative values counted from the end; not yet
/// checked against the bounds.
fn resolve_index(index: &Value, length: usize, position: Position) -> Result<f64, Error> {
    let Value::Number(number) = *index else {
        return Err(Error::at(
            position,
            Message::IndexNotNumber(index.value_type()),
        ));
    };
    if number.fract() != 0.0 {
        return Err(Error::at(position, Message::IndexNotWhole(number)));
    }
    Ok(if number < 0.0 {
        length as f64 + number
    } else {
        number
    })
}

/// `dilim(x, baş)` or `dilim(x, baş, son)`: the part of a list or text from
/// `baş` up to, but not including, `son`. Out-of-range bounds are clamped.
fn slice(
    arguments: &[Value],
    position: Position,
    wrong: impl Fn(&'static [Type], &Value) -> Error,
) -> Result<Value, Error> {
    let bound = |index: usize, length: usize, default: usize| -> Result<usize, Error> {
        match arguments.get(index) {
            None | Some(Value::Nil) => Ok(default),
            Some(value) => {
                Ok(resolve_index(value, length, position)?.clamp(0.0, length as f64) as usize)
            }
        }
    };
    match &arguments[0] {
        Value::List(items) => {
            let items = items.borrow();
            let start = bound(1, items.len(), 0)?;
            let end = bound(2, items.len(), items.len())?.max(start);
            Ok(Value::list(items[start..end].to_vec()))
        }
        Value::String(text) => {
            let chars: Vec<char> = text.chars().collect();
            let start = bound(1, chars.len(), 0)?;
            let end = bound(2, chars.len(), chars.len())?.max(start);
            Ok(Value::string(chars[start..end].iter().collect::<String>()))
        }
        other => Err(wrong(&[Type::List, Type::Text], other)),
    }
}

fn sort(items: &mut [Value], position: Position) -> Result<(), Error> {
    if items.iter().all(|item| matches!(item, Value::Number(_))) {
        items.sort_by(|a, b| match (a, b) {
            (Value::Number(a), Value::Number(b)) => a.partial_cmp(b).unwrap_or(Ordering::Equal),
            _ => Ordering::Equal,
        });
    } else if items.iter().all(|item| matches!(item, Value::String(_))) {
        items.sort_by(|a, b| match (a, b) {
            (Value::String(a), Value::String(b)) => a.cmp(b),
            _ => Ordering::Equal,
        });
    } else {
        return Err(Error::at(position, Message::MixedSort));
    }
    Ok(())
}

fn range(arguments: &[Value], position: Position) -> Result<Value, Error> {
    let mut numbers = Vec::with_capacity(arguments.len());
    for argument in arguments {
        match argument {
            Value::Number(number) => numbers.push(*number),
            other => {
                return Err(Error::at(
                    position,
                    Message::WrongArgument {
                        builtin: Builtin::Range,
                        expected: &[Type::Number],
                        given: other.value_type(),
                    },
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
        return Err(Error::at(position, Message::RangeStepZero));
    }

    let length = ((end - start) / step).ceil().max(0.0);
    if !length.is_finite() || length > MAX_RANGE_LENGTH as f64 {
        return Err(Error::at(position, Message::RangeTooLong(MAX_RANGE_LENGTH)));
    }
    let items = (0..length as usize)
        .map(|i| Value::Number(start + step * i as f64))
        .collect();
    Ok(Value::list(items))
}

/// Turkish has a dotted and a dotless i: `i` ↔ `İ` and `ı` ↔ `I`.
fn upper(text: &str, lang: Lang) -> String {
    match lang {
        Lang::Tr => text
            .chars()
            .flat_map(|c| match c {
                'i' => vec!['İ'],
                'ı' => vec!['I'],
                c => c.to_uppercase().collect(),
            })
            .collect(),
        Lang::En => text.to_uppercase(),
    }
}

fn lower(text: &str, lang: Lang) -> String {
    match lang {
        Lang::Tr => text
            .chars()
            .flat_map(|c| match c {
                'I' => vec!['ı'],
                'İ' => vec!['i'],
                c => c.to_lowercase().collect(),
            })
            .collect(),
        Lang::En => text.to_lowercase(),
    }
}

thread_local! {
    static RANDOM_STATE: Cell<u64> = Cell::new(seed());
}

fn seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_nanos() as u64)
        .unwrap_or(0x2545_f491_4f6c_dd1d);
    nanos | 1
}

/// A fraction in [0, 1) from a xorshift generator: plenty for games and
/// exercises, not for anything that needs to be unpredictable.
fn random_fraction() -> f64 {
    RANDOM_STATE.with(|state| {
        let mut x = state.get();
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        state.set(x);
        (x >> 11) as f64 / (1u64 << 53) as f64
    })
}

#[cfg(test)]
mod tests {
    use super::{lower, upper};
    use crate::keywords::Lang;

    #[test]
    fn changes_case_with_turkish_rules() {
        assert_eq!(upper("istanbul ılık", Lang::Tr), "İSTANBUL ILIK");
        assert_eq!(lower("İSTANBUL ILIK", Lang::Tr), "istanbul ılık");
        assert_eq!(upper("istanbul", Lang::En), "ISTANBUL");
    }
}
