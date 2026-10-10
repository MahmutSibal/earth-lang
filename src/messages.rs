//! Every message Earth shows to a person, in every language it speaks.
//!
//! Errors are built as [`Message`] values and turned into text only when
//! they are shown, so the same error can be reported in any [`Lang`].

use crate::keywords::{Builtin, Keyword, Lang};
use crate::value::format_number;

/// The kind of a value, as reported by `tür` and in type errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Type {
    Nil,
    Bool,
    Number,
    Text,
    List,
    Dict,
    Function,
}

impl Type {
    pub fn name(self, lang: Lang) -> &'static str {
        match (lang, self) {
            (Lang::Tr, Type::Nil) => "boş",
            (Lang::Tr, Type::Bool) => "mantıksal",
            (Lang::Tr, Type::Number) => "sayı",
            (Lang::Tr, Type::Text) => "metin",
            (Lang::Tr, Type::List) => "liste",
            (Lang::Tr, Type::Dict) => "sözlük",
            (Lang::Tr, Type::Function) => "fonksiyon",
            (Lang::En, Type::Nil) => "nil",
            (Lang::En, Type::Bool) => "bool",
            (Lang::En, Type::Number) => "number",
            (Lang::En, Type::Text) => "string",
            (Lang::En, Type::List) => "list",
            (Lang::En, Type::Dict) => "dict",
            (Lang::En, Type::Function) => "function",
        }
    }
}

/// What the parser found where it expected something else.
#[derive(Debug, Clone, PartialEq)]
pub enum Found {
    /// A symbol, identifier or number, shown as written.
    Token(String),
    Keyword(Keyword),
    Text,
    Newline,
    Eof,
}

impl Found {
    fn render(&self, lang: Lang) -> String {
        match (lang, self) {
            (_, Found::Token(token)) => format!("'{token}'"),
            (_, Found::Keyword(keyword)) => format!("'{}'", keyword.name(lang)),
            (Lang::Tr, Found::Text) => "metin".to_owned(),
            (Lang::Tr, Found::Newline) => "satır sonu".to_owned(),
            (Lang::Tr, Found::Eof) => "dosya sonu".to_owned(),
            (Lang::En, Found::Text) => "a string".to_owned(),
            (Lang::En, Found::Newline) => "end of line".to_owned(),
            (Lang::En, Found::Eof) => "end of file".to_owned(),
        }
    }
}

/// What the parser was looking for.
#[derive(Debug, Clone, PartialEq)]
pub enum Expected {
    /// A specific token, such as `')'` or `'içinde'`.
    Token(Found),
    VariableName,
    FunctionName,
    ParameterName,
    LoopVariable,
}

impl Expected {
    fn render(&self, lang: Lang) -> String {
        match (lang, self) {
            (_, Expected::Token(token)) => token.render(lang),
            (Lang::Tr, Expected::VariableName) => "değişken adı".to_owned(),
            (Lang::Tr, Expected::FunctionName) => "fonksiyon adı".to_owned(),
            (Lang::Tr, Expected::ParameterName) => "parametre adı".to_owned(),
            (Lang::Tr, Expected::LoopVariable) => "döngü değişkeni".to_owned(),
            (Lang::En, Expected::VariableName) => "a variable name".to_owned(),
            (Lang::En, Expected::FunctionName) => "a function name".to_owned(),
            (Lang::En, Expected::ParameterName) => "a parameter name".to_owned(),
            (Lang::En, Expected::LoopVariable) => "a loop variable".to_owned(),
        }
    }
}

/// The name of a callable, for arity errors.
#[derive(Debug, Clone, PartialEq)]
pub enum Callee {
    Builtin(Builtin),
    Function(String),
}

impl Callee {
    fn render(&self, lang: Lang) -> &str {
        match self {
            Callee::Builtin(builtin) => builtin.name(lang),
            Callee::Function(name) => name,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    // Lexer
    UnexpectedCharacter(char),
    InvalidEscape(char),
    UnterminatedString,
    InvalidNumber(String),

    // Parser
    Expected {
        expected: Expected,
        found: Found,
    },
    ExpectedStatementEnd(Found),
    ExpectedExpression(Found),
    OutsideFunction(Keyword),
    OutsideLoop(Keyword),
    DuplicateParameter(String),
    InvalidAssignmentTarget,
    UnclosedBlock,

    // Runtime
    UndefinedVariable(String),
    DivisionByZero,
    InvalidOperands {
        op: &'static str,
        left: Type,
        right: Type,
    },
    InvalidOperand {
        op: &'static str,
        operand: Type,
    },
    NotCallable(Type),
    NotIterable(Type),
    NotIndexable(Type),
    NotAssignable(Type),
    WrongArity {
        callee: Callee,
        expected: (usize, usize),
        given: usize,
    },
    RecursionLimit(usize),
    IndexNotNumber(Type),
    IndexNotWhole(f64),
    IndexOutOfRange {
        index: f64,
        length: usize,
    },
    MissingKey(String),
    KeyNotText(Type),
    NoLength(Type),
    WrongArgument {
        builtin: Builtin,
        expected: &'static [Type],
        given: Type,
    },
    RangeStepZero,
    RangeTooLong(usize),
    Io(String),
}

impl Message {
    pub fn render(&self, lang: Lang) -> String {
        match lang {
            Lang::Tr => self.turkish(),
            Lang::En => self.english(),
        }
    }

    fn turkish(&self) -> String {
        let lang = Lang::Tr;
        match self {
            Message::UnexpectedCharacter(c) => format!("Beklenmeyen karakter '{c}'"),
            Message::InvalidEscape(c) => format!("Geçersiz kaçış dizisi '\\{c}'"),
            Message::UnterminatedString => "Kapatılmamış metin".to_owned(),
            Message::InvalidNumber(text) => format!("Geçersiz sayı '{text}'"),

            Message::Expected { expected, found } => format!(
                "{} bekleniyordu, {} bulundu",
                expected.render(lang),
                found.render(lang)
            ),
            Message::ExpectedStatementEnd(found) => {
                format!("Satır sonu bekleniyordu, {} bulundu", found.render(lang))
            }
            Message::ExpectedExpression(found) => {
                format!("İfade bekleniyordu, {} bulundu", found.render(lang))
            }
            Message::OutsideFunction(keyword) => format!(
                "'{}' yalnızca bir fonksiyonun içinde kullanılabilir",
                keyword.name(lang)
            ),
            Message::OutsideLoop(keyword) => format!(
                "'{}' yalnızca bir döngünün içinde kullanılabilir",
                keyword.name(lang)
            ),
            Message::DuplicateParameter(name) => {
                format!("'{name}' parametresi iki kez tanımlanmış")
            }
            Message::InvalidAssignmentTarget => "Bu ifadeye değer atanamaz".to_owned(),
            Message::UnclosedBlock => "Blok kapatılmamış: '}' bekleniyordu".to_owned(),

            Message::UndefinedVariable(name) => format!(
                "Tanımsız değişken '{name}' (önce '{} {name} = ...' ile tanımlayın)",
                Keyword::Let.name(lang)
            ),
            Message::DivisionByZero => "Sıfıra bölme".to_owned(),
            Message::InvalidOperands { op, left, right } => format!(
                "'{op}' işlemi '{}' ve '{}' türleri arasında yapılamaz",
                left.name(lang),
                right.name(lang)
            ),
            Message::InvalidOperand { op, operand } => {
                format!("'{op}' işlemi '{}' türüne uygulanamaz", operand.name(lang))
            }
            Message::NotCallable(found) => {
                format!("'{}' türündeki bir değer çağrılamaz", found.name(lang))
            }
            Message::NotIterable(found) => format!(
                "'{}' türündeki bir değer üzerinde döngü kurulamaz",
                found.name(lang)
            ),
            Message::NotIndexable(found) => {
                format!("'{}' türündeki bir değer indekslenemez", found.name(lang))
            }
            Message::NotAssignable(found) => format!(
                "Yalnızca liste ve sözlük elemanlarına atama yapılabilir, {} bulundu",
                found.name(lang)
            ),
            Message::WrongArity {
                callee,
                expected,
                given,
            } => format!(
                "'{}' {} argüman bekliyor, {given} verildi",
                callee.render(lang),
                arity(*expected)
            ),
            Message::RecursionLimit(max) => {
                format!("Çok derin özyineleme (en fazla {max} iç içe çağrı)")
            }
            Message::IndexNotNumber(found) => {
                format!("İndeks sayı olmalı, {} bulundu", found.name(lang))
            }
            Message::IndexNotWhole(index) => {
                format!("İndeks tam sayı olmalı, {} bulundu", format_number(*index))
            }
            Message::IndexOutOfRange { index, length } => format!(
                "İndeks {} sınırların dışında (uzunluk {length})",
                format_number(*index)
            ),
            Message::MissingKey(key) => format!(
                "Sözlükte {key:?} anahtarı yok ('{}' ile kontrol edebilirsiniz)",
                Builtin::Contains.name(lang)
            ),
            Message::KeyNotText(found) => format!(
                "Sözlük anahtarı metin olmalı, {} bulundu (gerekirse {}(...) kullanın)",
                found.name(lang),
                Builtin::ToText.name(lang)
            ),
            Message::NoLength(found) => format!("'{}' türünün uzunluğu yoktur", found.name(lang)),
            Message::WrongArgument {
                builtin,
                expected,
                given,
            } => format!(
                "'{}' {} bekliyor, {} verildi",
                builtin.name(lang),
                types(expected, lang, "veya"),
                given.name(lang)
            ),
            Message::RangeStepZero => format!("'{}' adımı sıfır olamaz", Builtin::Range.name(lang)),
            Message::RangeTooLong(max) => format!(
                "'{}' en fazla {max} eleman üretebilir",
                Builtin::Range.name(lang)
            ),
            Message::Io(error) => format!("G/Ç hatası: {error}"),
        }
    }

    fn english(&self) -> String {
        let lang = Lang::En;
        match self {
            Message::UnexpectedCharacter(c) => format!("Unexpected character '{c}'"),
            Message::InvalidEscape(c) => format!("Invalid escape sequence '\\{c}'"),
            Message::UnterminatedString => "Unterminated string".to_owned(),
            Message::InvalidNumber(text) => format!("Invalid number '{text}'"),

            Message::Expected { expected, found } => format!(
                "Expected {}, found {}",
                expected.render(lang),
                found.render(lang)
            ),
            Message::ExpectedStatementEnd(found) => {
                format!("Expected end of line, found {}", found.render(lang))
            }
            Message::ExpectedExpression(found) => {
                format!("Expected an expression, found {}", found.render(lang))
            }
            Message::OutsideFunction(keyword) => {
                format!(
                    "'{}' can only be used inside a function",
                    keyword.name(lang)
                )
            }
            Message::OutsideLoop(keyword) => {
                format!("'{}' can only be used inside a loop", keyword.name(lang))
            }
            Message::DuplicateParameter(name) => format!("Parameter '{name}' is declared twice"),
            Message::InvalidAssignmentTarget => "Cannot assign to this expression".to_owned(),
            Message::UnclosedBlock => "Unclosed block: expected '}'".to_owned(),

            Message::UndefinedVariable(name) => format!(
                "Undefined variable '{name}' (declare it first with '{} {name} = ...')",
                Keyword::Let.name(lang)
            ),
            Message::DivisionByZero => "Division by zero".to_owned(),
            Message::InvalidOperands { op, left, right } => format!(
                "Cannot apply '{op}' to '{}' and '{}'",
                left.name(lang),
                right.name(lang)
            ),
            Message::InvalidOperand { op, operand } => {
                format!("Cannot apply '{op}' to '{}'", operand.name(lang))
            }
            Message::NotCallable(found) => {
                format!("A value of type '{}' cannot be called", found.name(lang))
            }
            Message::NotIterable(found) => {
                format!("Cannot loop over a value of type '{}'", found.name(lang))
            }
            Message::NotIndexable(found) => {
                format!("A value of type '{}' cannot be indexed", found.name(lang))
            }
            Message::NotAssignable(found) => format!(
                "Only list and dict elements can be assigned, found {}",
                found.name(lang)
            ),
            Message::WrongArity {
                callee,
                expected,
                given,
            } => format!(
                "'{}' expects {} argument{}, got {given}",
                callee.render(lang),
                arity(*expected),
                if *expected == (1, 1) { "" } else { "s" }
            ),
            Message::RecursionLimit(max) => {
                format!("Recursion too deep (at most {max} nested calls)")
            }
            Message::IndexNotNumber(found) => {
                format!("Index must be a number, found {}", found.name(lang))
            }
            Message::IndexNotWhole(index) => format!(
                "Index must be a whole number, found {}",
                format_number(*index)
            ),
            Message::IndexOutOfRange { index, length } => format!(
                "Index {} is out of range (length {length})",
                format_number(*index)
            ),
            Message::MissingKey(key) => format!(
                "Key {key:?} is not in the dict (check with '{}')",
                Builtin::Contains.name(lang)
            ),
            Message::KeyNotText(found) => format!(
                "Dict keys must be strings, found {} (use {}(...) if needed)",
                found.name(lang),
                Builtin::ToText.name(lang)
            ),
            Message::NoLength(found) => format!("Type '{}' has no length", found.name(lang)),
            Message::WrongArgument {
                builtin,
                expected,
                given,
            } => format!(
                "'{}' expects {}, got {}",
                builtin.name(lang),
                types(expected, lang, "or"),
                given.name(lang)
            ),
            Message::RangeStepZero => {
                format!("'{}' step cannot be zero", Builtin::Range.name(lang))
            }
            Message::RangeTooLong(max) => format!(
                "'{}' can produce at most {max} items",
                Builtin::Range.name(lang)
            ),
            Message::Io(error) => format!("I/O error: {error}"),
        }
    }
}

fn arity((min, max): (usize, usize)) -> String {
    match (min, max) {
        _ if min == max => min.to_string(),
        (_, usize::MAX) => format!("{min}+"),
        _ => format!("{min}-{max}"),
    }
}

/// `sayı`, `liste veya sözlük`, `liste, sözlük veya metin`.
fn types(types: &[Type], lang: Lang, or: &str) -> String {
    let names: Vec<&str> = types.iter().map(|t| t.name(lang)).collect();
    match names.split_last() {
        Some((last, [])) => (*last).to_owned(),
        Some((last, rest)) => format!("{} {or} {last}", rest.join(", ")),
        None => String::new(),
    }
}

/// Short phrases used by the command-line interface.
pub mod cli {
    use crate::keywords::Lang;

    pub fn error_prefix(lang: Lang) -> &'static str {
        match lang {
            Lang::Tr => "Earth hatası",
            Lang::En => "Earth error",
        }
    }

    pub fn usage(lang: Lang) -> &'static str {
        match lang {
            Lang::Tr => "Kullanım: earth [--dil tr|en] [kaynak-dosyası.earth]",
            Lang::En => "Usage: earth [--lang tr|en] [source-file.earth]",
        }
    }

    pub fn help(lang: Lang) -> &'static str {
        match lang {
            Lang::Tr => {
                "Dosya verilmezse etkileşimli kabuk (REPL) açılır.\n\
                 Çıktı dili programın anahtar kelimelerinden algılanır; --dil ile seçilebilir."
            }
            Lang::En => {
                "Without a file, an interactive shell (REPL) starts.\n\
                 The output language is detected from the program's keywords; --lang overrides it."
            }
        }
    }

    pub fn unknown_language(lang: Lang, code: &str) -> String {
        match lang {
            Lang::Tr => format!("Bilinmeyen dil '{code}' (tr veya en kullanın)"),
            Lang::En => format!("Unknown language '{code}' (use tr or en)"),
        }
    }

    pub fn cannot_read(lang: Lang, path: &str, error: &std::io::Error) -> String {
        match lang {
            Lang::Tr => format!("'{path}' okunamadı: {error}"),
            Lang::En => format!("Could not read '{path}': {error}"),
        }
    }

    pub fn repl_banner(lang: Lang, version: &str) -> String {
        match lang {
            Lang::Tr => format!("Earth {version} — çıkmak için Ctrl+D"),
            Lang::En => format!("Earth {version} — press Ctrl+D to exit"),
        }
    }

    pub fn repl_error(lang: Lang) -> &'static str {
        match lang {
            Lang::Tr => "Hata",
            Lang::En => "Error",
        }
    }
}
