//! The multilingual vocabulary of Earth.
//!
//! Every keyword and built-in function has one meaning and a column of
//! spellings per human language. Supporting a new language means adding a
//! [`Lang`] variant, a column to the tables below and its messages in
//! `messages.rs`; the lexer, parser and interpreter never see the words.

/// A human language Earth speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Lang {
    #[default]
    Tr,
    En,
}

impl Lang {
    pub const ALL: [Lang; 2] = [Lang::Tr, Lang::En];

    /// Parses a language code such as `tr` or `en`.
    pub fn from_code(code: &str) -> Option<Lang> {
        match code.to_lowercase().as_str() {
            "tr" | "türkçe" | "turkce" | "turkish" => Some(Lang::Tr),
            "en" | "ingilizce" | "english" => Some(Lang::En),
            _ => None,
        }
    }

    fn column(self) -> usize {
        self as usize
    }
}

/// One spelling list per language, in the order of [`Lang::ALL`]. The first
/// spelling in each list is the one Earth uses when it prints the word.
type Spellings = [&'static [&'static str]; 2];

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

/// Turkish also accepts ASCII-only spellings for keyboards without Turkish
/// letters.
const KEYWORDS: &[(Keyword, Spellings)] = &[
    (Keyword::Let, [&["değişken", "degisken"], &["let"]]),
    (Keyword::If, [&["eğer", "eger"], &["if"]]),
    (Keyword::Else, [&["değilse", "degilse"], &["else"]]),
    (Keyword::While, [&["iken"], &["while"]]),
    (Keyword::For, [&["her"], &["for"]]),
    (Keyword::In, [&["içinde", "icinde"], &["in"]]),
    (Keyword::Function, [&["fonksiyon"], &["function", "fn"]]),
    (Keyword::Return, [&["döndür", "dondur"], &["return"]]),
    (Keyword::True, [&["doğru", "dogru"], &["true"]]),
    (Keyword::False, [&["yanlış", "yanlis"], &["false"]]),
    (Keyword::Nil, [&["boş", "bos"], &["nil"]]),
    (Keyword::And, [&["ve"], &["and"]]),
    (Keyword::Or, [&["veya"], &["or"]]),
    (Keyword::Not, [&["değil", "degil"], &["not"]]),
    (Keyword::Break, [&["kır", "kir"], &["break"]]),
    (Keyword::Continue, [&["devam"], &["continue"]]),
];

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
    Upper,
    Lower,
    Trim,
    Split,
    Join,
    Replace,
    Abs,
    Sqrt,
    Power,
    Round,
    Floor,
    Ceil,
    Random,
    Min,
    Max,
    Sum,
    Sort,
    Reverse,
    Slice,
}

const BUILTINS: &[(Builtin, Spellings)] = &[
    (Builtin::Print, [&["yaz"], &["print"]]),
    (Builtin::Input, [&["girdi"], &["input"]]),
    (Builtin::Length, [&["uzunluk"], &["len"]]),
    (Builtin::Range, [&["aralık", "aralik"], &["range"]]),
    (Builtin::ToText, [&["metin"], &["str"]]),
    (Builtin::ToNumber, [&["sayı", "sayi"], &["number"]]),
    (Builtin::Push, [&["ekle"], &["push"]]),
    (Builtin::Type, [&["tür", "tur"], &["type"]]),
    (Builtin::Keys, [&["anahtarlar"], &["keys"]]),
    (Builtin::Values, [&["değerler", "degerler"], &["values"]]),
    (Builtin::Contains, [&["içerir", "icerir"], &["contains"]]),
    (Builtin::Remove, [&["sil"], &["remove"]]),
    (Builtin::Upper, [&["büyük_harf", "buyuk_harf"], &["upper"]]),
    (Builtin::Lower, [&["küçük_harf", "kucuk_harf"], &["lower"]]),
    (Builtin::Trim, [&["kırp", "kirp"], &["trim"]]),
    (Builtin::Split, [&["böl", "bol"], &["split"]]),
    (Builtin::Join, [&["birleştir", "birlestir"], &["join"]]),
    (Builtin::Replace, [&["değiştir", "degistir"], &["replace"]]),
    (Builtin::Abs, [&["mutlak"], &["abs"]]),
    (Builtin::Sqrt, [&["karekök", "karekok"], &["sqrt"]]),
    (Builtin::Power, [&["üs", "us"], &["pow"]]),
    (Builtin::Round, [&["yuvarla"], &["round"]]),
    (Builtin::Floor, [&["taban"], &["floor"]]),
    (Builtin::Ceil, [&["tavan"], &["ceil"]]),
    (Builtin::Random, [&["rastgele"], &["random"]]),
    (Builtin::Min, [&["en_küçük", "en_kucuk"], &["min"]]),
    (Builtin::Max, [&["en_büyük", "en_buyuk"], &["max"]]),
    (Builtin::Sum, [&["topla"], &["sum"]]),
    (Builtin::Sort, [&["sırala", "sirala"], &["sort"]]),
    (Builtin::Reverse, [&["ters"], &["reverse"]]),
    (Builtin::Slice, [&["dilim"], &["slice"]]),
];

fn lookup<T: Copy>(table: &[(T, Spellings)], word: &str) -> Option<(T, Lang)> {
    table.iter().find_map(|(meaning, spellings)| {
        Lang::ALL
            .into_iter()
            .find(|lang| spellings[lang.column()].contains(&word))
            .map(|lang| (*meaning, lang))
    })
}

fn spelling<T: PartialEq>(table: &[(T, Spellings)], meaning: &T, lang: Lang) -> &'static str {
    table
        .iter()
        .find(|(candidate, _)| candidate == meaning)
        .map(|(_, spellings)| spellings[lang.column()][0])
        .expect("every word has a spelling in every language")
}

pub fn keyword(word: &str) -> Option<Keyword> {
    lookup(KEYWORDS, word).map(|(keyword, _)| keyword)
}

/// The language a keyword or built-in spelling belongs to, if any.
pub fn language_of(word: &str) -> Option<Lang> {
    lookup(KEYWORDS, word)
        .map(|(_, lang)| lang)
        .or_else(|| lookup(BUILTINS, word).map(|(_, lang)| lang))
}

/// Every (spelling, builtin) pair, used to populate the global scope.
pub fn builtins() -> impl Iterator<Item = (&'static str, Builtin)> {
    BUILTINS.iter().flat_map(|(builtin, spellings)| {
        spellings
            .iter()
            .flat_map(|column| column.iter())
            .map(move |spelling| (*spelling, *builtin))
    })
}

impl Keyword {
    pub fn name(self, lang: Lang) -> &'static str {
        spelling(KEYWORDS, &self, lang)
    }
}

impl Builtin {
    pub fn name(self, lang: Lang) -> &'static str {
        spelling(BUILTINS, &self, lang)
    }
}

#[cfg(test)]
mod tests {
    use super::{keyword, language_of, Keyword, Lang, BUILTINS, KEYWORDS};

    #[test]
    fn every_spelling_maps_to_one_keyword() {
        assert_eq!(keyword("eğer"), Some(Keyword::If));
        assert_eq!(keyword("eger"), Some(Keyword::If));
        assert_eq!(keyword("if"), Some(Keyword::If));
        assert_eq!(keyword("x"), None);
    }

    #[test]
    fn knows_the_language_of_each_spelling() {
        assert_eq!(language_of("değişken"), Some(Lang::Tr));
        assert_eq!(language_of("yaz"), Some(Lang::Tr));
        assert_eq!(language_of("while"), Some(Lang::En));
        assert_eq!(language_of("print"), Some(Lang::En));
        assert_eq!(language_of("x"), None);
        assert_eq!(Keyword::Nil.name(Lang::En), "nil");
    }

    #[test]
    fn spellings_are_unique_and_complete() {
        let mut seen = std::collections::HashSet::new();
        let keyword_spellings = KEYWORDS.iter().map(|(_, spellings)| spellings);
        let builtin_spellings = BUILTINS.iter().map(|(_, spellings)| spellings);
        for columns in keyword_spellings.chain(builtin_spellings) {
            for column in columns {
                assert!(!column.is_empty(), "a language is missing a spelling");
                for spelling in *column {
                    assert!(seen.insert(*spelling), "duplicate spelling {spelling}");
                }
            }
        }
    }
}
