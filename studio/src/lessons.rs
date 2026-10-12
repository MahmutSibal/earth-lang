//! The teacher's course: lessons in the order of the README tutorial,
//! ending with two small projects. Each lesson names what it covers, so
//! the teacher can explain it, show an example and set an exercise.

use earth::Lang;

pub struct Lesson {
    pub title_tr: &'static str,
    pub title_en: &'static str,
    /// What the lesson covers, for the teacher (in English).
    pub topics: &'static str,
}

impl Lesson {
    pub fn title(&self, lang: Lang) -> &'static str {
        match lang {
            Lang::Tr => self.title_tr,
            Lang::En => self.title_en,
        }
    }
}

pub const LESSONS: &[Lesson] = &[
    Lesson {
        title_tr: "İlk program",
        title_en: "First program",
        topics: "yaz/print, running a program with F5, comments with #",
    },
    Lesson {
        title_tr: "Değişkenler",
        title_en: "Variables",
        topics: "değişken/let to create, = to change, naming, using variables in yaz",
    },
    Lesson {
        title_tr: "Veri türleri",
        title_en: "Types of values",
        topics: "numbers, text, doğru/yanlış, boş, tür(), metin() and sayı() conversions",
    },
    Lesson {
        title_tr: "İşlemler",
        title_en: "Operators",
        topics: "+ - * / %, comparisons, ve/veya/değil, precedence and parentheses",
    },
    Lesson {
        title_tr: "Metinler",
        title_en: "Text",
        topics: "joining text with +, uzunluk, büyük_harf, küçük_harf, kırp, böl, birleştir, değiştir, indexing",
    },
    Lesson {
        title_tr: "Koşullar",
        title_en: "Conditions",
        topics: "eğer / değilse / değilse eğer, comparing values, nested conditions",
    },
    Lesson {
        title_tr: "Döngüler",
        title_en: "Loops",
        topics: "iken/while loops, her … içinde aralık(…) loops, kır and devam, counting and summing",
    },
    Lesson {
        title_tr: "Listeler",
        title_en: "Lists",
        topics: "[ ], indexing, ekle, sil, uzunluk, looping over a list, sırala, topla",
    },
    Lesson {
        title_tr: "Sözlükler",
        title_en: "Dictionaries",
        topics: "{ \"key\": value }, reading and changing keys, anahtarlar, değerler, içerir",
    },
    Lesson {
        title_tr: "Fonksiyonlar",
        title_en: "Functions",
        topics: "fonksiyon, parameters, döndür, calling functions, recursion",
    },
    Lesson {
        title_tr: "Kullanıcıdan girdi",
        title_en: "Input",
        topics: "girdi() to ask the user, converting input with sayı(), checking for invalid input",
    },
    Lesson {
        title_tr: "Hata yakalama",
        title_en: "Handling errors",
        topics: "dene / yakala, fırlat, reading Earth's error messages",
    },
    Lesson {
        title_tr: "Modüller",
        title_en: "Modules",
        topics: "içe_aktar to use another .earth file, splitting a program into files",
    },
    Lesson {
        title_tr: "Proje: Sayı tahmin oyunu",
        title_en: "Project: Guess the number",
        topics: "a project combining rastgele, girdi, sayı, loops and conditions; build it step by step",
    },
    Lesson {
        title_tr: "Proje: Not defteri",
        title_en: "Project: Grade book",
        topics: "a project combining lists, dictionaries, functions and loops: store students' grades and print averages",
    },
];

#[cfg(test)]
mod tests {
    use super::LESSONS;

    #[test]
    fn every_lesson_has_both_titles() {
        assert!(LESSONS.len() >= 13);
        for lesson in LESSONS {
            assert!(!lesson.title_tr.is_empty() && !lesson.title_en.is_empty());
            assert!(!lesson.topics.is_empty());
        }
    }
}
