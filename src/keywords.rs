//! The multilingual vocabulary of Earth.
//!
//! Every keyword and built-in function has one meaning and many spellings.
//! Supporting a new human language means adding its spellings to the tables
//! below; the lexer, parser and interpreter never see the words themselves.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Keyword {
    Let,
    If,
    Else,
    While,
    For,
    In,
    Function,
    Return,
    True,
    False,
    Nil,
    And,
    Or,
    Not,
    Break,
    Continue,
}

/// Keyword spellings. Turkish first, then ASCII-only Turkish for keyboards
/// without Turkish letters, then English.
const KEYWORDS: &[(Keyword, &[&str])] = &[
    (Keyword::Let, &["değişken", "degisken", "let"]),
    (Keyword::If, &["eğer", "eger", "if"]),
    (Keyword::Else, &["değilse", "degilse", "else"]),
    (Keyword::While, &["iken", "while"]),
    (Keyword::For, &["her", "for"]),
    (Keyword::In, &["içinde", "icinde", "in"]),
    (Keyword::Function, &["fonksiyon", "function", "fn"]),
    (Keyword::Return, &["döndür", "dondur", "return"]),
    (Keyword::True, &["doğru", "dogru", "true"]),
    (Keyword::False, &["yanlış", "yanlis", "false"]),
    (Keyword::Nil, &["boş", "bos", "nil"]),
    (Keyword::And, &["ve", "and"]),
    (Keyword::Or, &["veya", "or"]),
    (Keyword::Not, &["değil", "degil", "not"]),
    (Keyword::Break, &["kır", "kir", "break"]),
    (Keyword::Continue, &["devam", "continue"]),
];

pub fn keyword(word: &str) -> Option<Keyword> {
    KEYWORDS
        .iter()
        .find(|(_, spellings)| spellings.contains(&word))
        .map(|(keyword, _)| *keyword)
}

impl Keyword {
    /// The canonical (Turkish) spelling, used in diagnostics.
    pub fn name(self) -> &'static str {
        KEYWORDS
            .iter()
            .find(|(keyword, _)| *keyword == self)
            .map(|(_, spellings)| spellings[0])
            .expect("every keyword has a spelling")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Builtin {
    Print,
    Input,
    Length,
    Range,
    ToText,
    ToNumber,
    Push,
    Type,
    Keys,
    Values,
    Contains,
    Remove,
}

const BUILTINS: &[(Builtin, &[&str])] = &[
    (Builtin::Print, &["yaz", "print"]),
    (Builtin::Input, &["girdi", "input"]),
    (Builtin::Length, &["uzunluk", "len"]),
    (Builtin::Range, &["aralık", "aralik", "range"]),
    (Builtin::ToText, &["metin", "str"]),
    (Builtin::ToNumber, &["sayı", "sayi", "number"]),
    (Builtin::Push, &["ekle", "push"]),
    (Builtin::Type, &["tür", "tur", "type"]),
    (Builtin::Keys, &["anahtarlar", "keys"]),
    (Builtin::Values, &["değerler", "degerler", "values"]),
    (Builtin::Contains, &["içerir", "icerir", "contains"]),
    (Builtin::Remove, &["sil", "remove"]),
];

/// Every (spelling, builtin) pair, used to populate the global scope.
pub fn builtins() -> impl Iterator<Item = (&'static str, Builtin)> {
    BUILTINS.iter().flat_map(|(builtin, spellings)| {
        spellings.iter().map(move |spelling| (*spelling, *builtin))
    })
}

impl Builtin {
    pub fn name(self) -> &'static str {
        BUILTINS
            .iter()
            .find(|(builtin, _)| *builtin == self)
            .map(|(_, spellings)| spellings[0])
            .expect("every builtin has a spelling")
    }
}

#[cfg(test)]
mod tests {
    use super::{keyword, Keyword, BUILTINS, KEYWORDS};

    #[test]
    fn every_spelling_maps_to_one_keyword() {
        assert_eq!(keyword("eğer"), Some(Keyword::If));
        assert_eq!(keyword("eger"), Some(Keyword::If));
        assert_eq!(keyword("if"), Some(Keyword::If));
        assert_eq!(keyword("x"), None);
    }

    #[test]
    fn spellings_are_unique() {
        let mut seen = std::collections::HashSet::new();
        let keyword_spellings = KEYWORDS.iter().map(|(_, spellings)| *spellings);
        let builtin_spellings = BUILTINS.iter().map(|(_, spellings)| *spellings);
        for spellings in keyword_spellings.chain(builtin_spellings) {
            for spelling in spellings {
                assert!(seen.insert(*spelling), "duplicate spelling {spelling}");
            }
        }
    }
}
