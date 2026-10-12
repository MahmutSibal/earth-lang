//! Suggestions while typing: Earth's keywords and built-in functions in the
//! program's language, and the names the program itself defines. Matching
//! ignores case and Turkish letters, so `degis` finds `değişken`.

use earth::keywords::{all_keywords, builtins, language_of, Keyword};
use earth::Lang;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Kind {
    Keyword,
    Builtin,
    /// A function defined in the program.
    Function,
    /// Any other name used in the program.
    Name,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Suggestion {
    /// The word as shown.
    pub word: String,
    pub kind: Kind,
}

impl Suggestion {
    /// What replaces the typed prefix, and where the cursor goes in it
    /// (in characters): after a space for most keywords, between the
    /// parentheses for functions.
    pub fn insertion(&self) -> (String, usize) {
        match self.kind {
            Kind::Builtin | Kind::Function => {
                let text = format!("{}()", self.word);
                let cursor = text.chars().count() - 1;
                (text, cursor)
            }
            Kind::Keyword if takes_space(&self.word) => {
                let text = format!("{} ", self.word);
                let cursor = text.chars().count();
                (text, cursor)
            }
            _ => (self.word.clone(), self.word.chars().count()),
        }
    }
}

fn takes_space(word: &str) -> bool {
    !matches!(
        earth::keywords::keyword(word),
        Some(Keyword::True | Keyword::False | Keyword::Nil | Keyword::Break | Keyword::Continue)
    )
}

/// The identifier being typed just before the cursor (a character index):
/// where it starts (a character index) and its text.
pub fn word_before(text: &str, cursor: usize) -> Option<(usize, String)> {
    let chars: Vec<char> = text.chars().take(cursor).collect();
    if chars.len() < cursor {
        return None;
    }
    let mut start = chars.len();
    while start > 0 && is_word_char(chars[start - 1]) {
        start -= 1;
    }
    let word: String = chars[start..].iter().collect();
    // Not inside a number, a string or a comment.
    if word.is_empty() || word.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        return None;
    }
    let line_start = chars[..start]
        .iter()
        .rposition(|&c| c == '\n')
        .map_or(0, |index| index + 1);
    let before: String = chars[line_start..start].iter().collect();
    if before.contains('#') || before.matches('"').count() % 2 == 1 {
        return None;
    }
    Some((start, word))
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Lowercase without Turkish letters: `Değişken` → `degisken`.
pub fn fold(text: &str) -> String {
    text.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'ç' => 'c',
            'ğ' => 'g',
            'ı' => 'i',
            'ö' => 'o',
            'ş' => 's',
            'ü' => 'u',
            '\u{307}' => '\0',
            other => other,
        })
        .filter(|&c| c != '\0')
        .collect()
}

/// Up to `limit` suggestions for `prefix` in `source`, best first.
pub fn suggestions(source: &str, prefix: &str, lang: Lang, limit: usize) -> Vec<Suggestion> {
    let folded = fold(prefix);
    if folded.is_empty() {
        return Vec::new();
    }
    let mut candidates: Vec<Suggestion> = Vec::new();
    let mut add = |word: String, kind: Kind| {
        if word != prefix
            && fold(&word).starts_with(&folded)
            && !candidates.iter().any(|known| known.word == word)
        {
            candidates.push(Suggestion { word, kind });
        }
    };
    for (word, kind) in program_names(source, prefix) {
        add(word, kind);
    }
    for keyword in all_keywords() {
        add(keyword.name(lang).to_owned(), Kind::Keyword);
    }
    for (spelling, builtin) in builtins() {
        // One spelling per built-in: the program language's main one.
        if builtin.name(lang) == spelling && language_of(spelling) == Some(lang) {
            add(spelling.to_owned(), Kind::Builtin);
        }
    }
    // Exact (unfolded) prefix matches first, then the program's own names,
    // keywords and built-ins, shorter words first in each.
    candidates.sort_by_key(|suggestion| {
        let rank = match suggestion.kind {
            Kind::Name | Kind::Function => 0,
            Kind::Keyword => 1,
            Kind::Builtin => 2,
        };
        (
            !suggestion.word.starts_with(prefix),
            rank,
            suggestion.word.chars().count(),
        )
    });
    candidates.truncate(limit);
    candidates
}

/// The names a program uses, functions marked, without the word being
/// typed (it would otherwise suggest itself).
fn program_names(source: &str, typing: &str) -> Vec<(String, Kind)> {
    let mut names: Vec<(String, Kind)> = Vec::new();
    let mut previous = String::new();
    let mut typing_seen = false;
    for line in source.lines() {
        let code = line.split('#').next().unwrap_or_default();
        let mut in_string = false;
        let mut word = String::new();
        for c in code.chars().chain(std::iter::once(' ')) {
            if c == '"' {
                in_string = !in_string;
                word.clear();
                continue;
            }
            if in_string {
                continue;
            }
            if is_word_char(c) {
                word.push(c);
                continue;
            }
            if word.is_empty() {
                continue;
            }
            let is_keyword = earth::keywords::keyword(&word).is_some();
            let is_builtin = language_of(&word).is_some();
            let starts_with_digit = word.chars().next().is_some_and(|c| c.is_ascii_digit());
            if word == typing && !typing_seen {
                typing_seen = true;
            } else if !is_keyword && !is_builtin && !starts_with_digit && word.chars().count() > 1 {
                let function = earth::keywords::keyword(&previous) == Some(Keyword::Function);
                match names.iter_mut().find(|(known, _)| *known == word) {
                    Some((_, kind)) if function => *kind = Kind::Function,
                    Some(_) => {}
                    None => names.push((
                        word.clone(),
                        if function { Kind::Function } else { Kind::Name },
                    )),
                }
            }
            previous = std::mem::take(&mut word);
        }
    }
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(source: &str, prefix: &str, lang: Lang) -> Vec<String> {
        suggestions(source, prefix, lang, 8)
            .into_iter()
            .map(|s| s.word)
            .collect()
    }

    #[test]
    fn finds_keywords_without_turkish_letters() {
        assert_eq!(
            words("degis", "degis", Lang::Tr),
            vec!["değişken", "değiştir"]
        );
        assert_eq!(words("Eğe", "Eğe", Lang::Tr), vec!["eğer"]);
        assert_eq!(words("ret", "ret", Lang::En), vec!["return"]);
    }

    #[test]
    fn suggests_builtins_in_the_program_language() {
        assert!(words("ya", "ya", Lang::Tr).contains(&"yaz".to_owned()));
        assert!(words("pr", "pr", Lang::En).contains(&"print".to_owned()));
        // English names are not offered to a Turkish program.
        assert!(!words("pr", "pr", Lang::Tr).contains(&"print".to_owned()));
        // ASCII spellings are not offered separately.
        assert_eq!(words("buy", "buy", Lang::Tr), vec!["büyük_harf"]);
    }

    #[test]
    fn suggests_the_programs_own_names_first() {
        let source = "değişken toplam = 0\nfonksiyon topla_hepsi(l) { döndür 0 }\ntop";
        let found = suggestions(source, "top", Lang::Tr, 8);
        assert_eq!(found[0].word, "toplam");
        assert_eq!(found[0].kind, Kind::Name);
        assert!(found
            .iter()
            .any(|s| s.word == "topla" && s.kind == Kind::Builtin));
        let function = found.iter().find(|s| s.word == "topla_hepsi").unwrap();
        assert_eq!(function.kind, Kind::Function);
        // Words inside strings and comments are not names.
        assert!(words("yaz(\"tamam\") # tamamen\nta", "ta", Lang::Tr)
            .iter()
            .all(|w| !w.starts_with("tamam")));
    }

    #[test]
    fn finds_the_word_before_the_cursor() {
        assert_eq!(word_before("yaz(deg", 7), Some((4, "deg".to_owned())));
        assert_eq!(word_before("değ", 3), Some((0, "değ".to_owned())));
        assert_eq!(word_before("yaz(\"deg", 8), None);
        assert_eq!(word_before("# deg", 5), None);
        assert_eq!(word_before("x = 12", 6), None);
        assert_eq!(word_before("yaz(", 4), None);
    }

    #[test]
    fn inserts_parentheses_and_spaces() {
        let builtin = Suggestion {
            word: "yaz".to_owned(),
            kind: Kind::Builtin,
        };
        assert_eq!(builtin.insertion(), ("yaz()".to_owned(), 4));
        let keyword = Suggestion {
            word: "değişken".to_owned(),
            kind: Kind::Keyword,
        };
        assert_eq!(keyword.insertion(), ("değişken ".to_owned(), 9));
        let constant = Suggestion {
            word: "doğru".to_owned(),
            kind: Kind::Keyword,
        };
        assert_eq!(constant.insertion(), ("doğru".to_owned(), 5));
    }
}
