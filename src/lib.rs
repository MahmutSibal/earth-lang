//! Earth — Kendi Dünyan, Kendi Kodlaman.
//!
//! The pipeline is `lexer` → `parser` → `interpreter`. Keywords and built-in
//! names in every supported human language live in `keywords`.

pub mod ast;
pub mod debug;
pub mod error;
pub mod interpreter;
pub mod keywords;
pub mod lexer;
pub mod messages;
pub mod parser;
pub mod stdlib;
pub mod value;

use std::io::{BufRead, Write};

pub use error::{Error, Position};
pub use interpreter::Interpreter;
pub use keywords::Lang;

/// Parses a whole source file.
pub fn parse(source: &str) -> Result<ast::Program, Error> {
    parser::parse(&lexer::lex(source)?)
}

/// The language a program is written in, judged by its keywords and
/// built-in names. Earth answers in that language.
pub fn detect_language(source: &str) -> Lang {
    lexer::lex(source)
        .map(|tokens| lexer::detect_language(&tokens))
        .unwrap_or_default()
}

/// Runs a program, streaming its output to `output` and reading `girdi`
/// from `input`. Values such as `doğru`/`true` are printed in `lang`.
pub fn run_with_io(
    source: &str,
    output: &mut dyn Write,
    input: &mut dyn BufRead,
    lang: Lang,
) -> Result<(), Error> {
    let program = parse(source)?;
    Interpreter::new(output, input, lang).run(&program)?;
    Ok(())
}

/// Runs a program with no input, in its detected language, and returns
/// everything it printed.
pub fn run(source: &str) -> Result<String, Error> {
    let mut output = Vec::new();
    run_with_io(
        source,
        &mut output,
        &mut std::io::empty(),
        detect_language(source),
    )?;
    Ok(String::from_utf8(output).expect("Earth only writes UTF-8"))
}

#[cfg(test)]
mod tests {
    use super::{run, run_with_io, Error, Lang, Position};

    fn run_error(source: &str) -> Error {
        run(source).unwrap_err()
    }

    #[test]
    fn runs_turkish_and_english_print_aliases() {
        assert_eq!(
            run("yaz(\"Merhaba, Earth!\")\nprint(\"Hello, Earth!\")").unwrap(),
            "Merhaba, Earth!\nHello, Earth!\n"
        );
    }

    #[test]
    fn mixes_languages_in_one_program() {
        let source = "\
let sayaç = 0
iken sayaç < 3 {
    sayaç = sayaç + 1
}
if sayaç == 3 { yaz(\"tamam\") } else { print(\"no\") }";
        assert_eq!(run(source).unwrap(), "tamam\n");
    }

    #[test]
    fn evaluates_arithmetic_and_strings() {
        assert_eq!(
            run("yaz(1 + 2 * 3, (1 + 2) * 3, 7 % 3, 1 / 4, -2)").unwrap(),
            "7 9 1 0.25 -2\n"
        );
        assert_eq!(run("yaz(\"x = \" + 5)").unwrap(), "x = 5\n");
    }

    #[test]
    fn supports_else_if_chains() {
        let source = "\
fonksiyon not_hesapla(puan) {
    eğer puan >= 85 { döndür \"AA\" }
    değilse eğer puan >= 70 { döndür \"BA\" }
    değilse { döndür \"FF\" }
}
yaz(not_hesapla(90), not_hesapla(75), not_hesapla(10))";
        assert_eq!(run(source).unwrap(), "AA BA FF\n");
    }

    #[test]
    fn supports_recursion_and_closures() {
        let source = "\
fonksiyon fib(n) {
    eğer n < 2 { döndür n }
    döndür fib(n - 1) + fib(n - 2)
}
fonksiyon sayaç_yap() {
    değişken n = 0
    fonksiyon artır() {
        n = n + 1
        döndür n
    }
    döndür artır
}
değişken say = sayaç_yap()
say()
yaz(fib(15), say())";
        assert_eq!(run(source).unwrap(), "610 2\n");
    }

    #[test]
    fn supports_lists_loops_break_and_continue() {
        let source = "\
değişken liste = []
her i içinde aralık(10) {
    eğer i % 2 == 0 { devam }
    eğer i > 7 { kır }
    ekle(liste, i)
}
liste[0] = 100
yaz(liste, uzunluk(liste), liste[-1])";
        assert_eq!(run(source).unwrap(), "[100, 3, 5, 7] 4 7\n");
    }

    #[test]
    fn short_circuits_logical_operators() {
        assert_eq!(
            run("yaz(yanlış ve tanımsız(), doğru veya tanımsız(), değil boş)").unwrap(),
            "yanlış doğru doğru\n"
        );
    }

    #[test]
    fn scopes_block_variables() {
        let error = run_error("eğer doğru { değişken iç = 1 }\nyaz(iç)");
        assert_eq!(error.position, Some(Position::new(2, 5)));
        assert!(error.text(Lang::Tr).starts_with("Tanımsız değişken 'iç'"));
    }

    #[test]
    fn reports_runtime_errors_with_positions() {
        let error = run_error("yaz(1)\ndeğişken x = 1 / 0");
        assert_eq!(error.to_string(), "2:16: Sıfıra bölme");

        let error = run_error("yaz([1, 2][5])");
        assert_eq!(
            error.text(Lang::Tr),
            "İndeks 5 sınırların dışında (uzunluk 2)"
        );

        let error = run_error("yaz(1 + doğru)");
        assert_eq!(
            error.text(Lang::Tr),
            "'+' işlemi 'sayı' ve 'mantıksal' türleri arasında yapılamaz"
        );
    }

    #[test]
    fn checks_function_arity() {
        let error = run_error("fonksiyon f(a, b) { }\nf(1)");
        assert_eq!(error.text(Lang::Tr), "'f' 2 argüman bekliyor, 1 verildi");
    }

    #[test]
    fn stops_runaway_recursion() {
        // Debug builds use a lot of stack per Earth call; give the test room.
        let handle = std::thread::Builder::new()
            .stack_size(256 * 1024 * 1024)
            .spawn(|| {
                run("fonksiyon f() { f() }\nf()")
                    .unwrap_err()
                    .text(Lang::Tr)
            })
            .unwrap();
        let message = handle.join().unwrap();
        assert!(message.starts_with("Çok derin özyineleme"));
    }

    #[test]
    fn reads_input() {
        let mut output = Vec::new();
        let mut input = "Ada\n".as_bytes();
        run_with_io(
            "değişken ad = girdi(\"Adın? \")\nyaz(\"Merhaba \" + ad)\nyaz(girdi())",
            &mut output,
            &mut input,
            Lang::Tr,
        )
        .unwrap();
        assert_eq!(
            String::from_utf8(output).unwrap(),
            "Adın? Merhaba Ada\nboş\n"
        );
    }

    #[test]
    fn converts_between_types() {
        assert_eq!(
            run("yaz(sayı(\" 42 \") + 1, sayı(\"abc\"), metin(3) + metin(doğru), tür([]))")
                .unwrap(),
            "43 boş 3doğru liste\n"
        );
    }

    #[test]
    fn supports_dictionaries() {
        let source = "\
değişken kişi = {
    \"ad\": \"Ada\",
    \"yaş\": 36,
}
kişi[\"şehir\"] = \"Londra\"
kişi[\"yaş\"] = kişi[\"yaş\"] + 1
yaz(kişi)
yaz(uzunluk(kişi), içerir(kişi, \"ad\"), içerir(kişi, \"soyad\"))
yaz(sil(kişi, \"şehir\"), anahtarlar(kişi), değerler(kişi))
her k içinde kişi { yaz(k + \"=\" + kişi[k]) }
yaz({} == {}, {\"a\": 1, \"b\": 2} == {\"b\": 2, \"a\": 1})";
        assert_eq!(
            run(source).unwrap(),
            "{\"ad\": \"Ada\", \"yaş\": 37, \"şehir\": \"Londra\"}\n\
             3 doğru yanlış\n\
             Londra [\"ad\", \"yaş\"] [\"Ada\", 37]\n\
             ad=Ada\n\
             yaş=37\n\
             doğru doğru\n"
        );
    }

    #[test]
    fn contains_and_remove_work_on_lists_and_text() {
        assert_eq!(
            run("değişken l = [1, 2, 3]\nyaz(sil(l, 0), l, içerir(l, 3), içerir(\"Merhaba\", \"hab\"))")
                .unwrap(),
            "1 [2, 3] doğru doğru\n"
        );
    }

    #[test]
    fn reports_dictionary_errors() {
        let error = run_error("değişken s = {\"a\": 1}\nyaz(s[\"b\"])");
        assert_eq!(error.position, Some(Position::new(2, 6)));
        assert!(error
            .text(Lang::Tr)
            .starts_with("Sözlükte \"b\" anahtarı yok"));

        let error = run_error("değişken s = {1: 2}");
        assert_eq!(error.position, Some(Position::new(1, 15)));
        assert!(error
            .text(Lang::Tr)
            .starts_with("Sözlük anahtarı metin olmalı"));
    }

    #[test]
    fn answers_in_the_language_of_the_program() {
        assert_eq!(
            run("print(1 == 1, nil, type([]))").unwrap(),
            "true nil list\n"
        );
        assert_eq!(
            run("yaz(1 == 1, boş, tür([]))").unwrap(),
            "doğru boş liste\n"
        );

        let error = run_error("let x = 1\nprint(x + true)");
        assert_eq!(
            error.render(Lang::En),
            "2:9: Cannot apply '+' to 'number' and 'bool'"
        );
        let error = run_error("if true {\n  break\n}");
        assert_eq!(
            error.text(Lang::En),
            "'break' can only be used inside a loop"
        );
        let error = run_error("print(len(1, 2))");
        assert_eq!(error.text(Lang::En), "'len' expects 1 argument, got 2");
    }

    #[test]
    fn provides_text_functions() {
        assert_eq!(
            run("yaz(büyük_harf(\"iğdır\"), küçük_harf(\"IŞIK\"), kırp(\"  a  \") + \"|\")")
                .unwrap(),
            "İĞDIR ışık a|\n"
        );
        assert_eq!(
            run("yaz(böl(\"a,b,c\", \",\"), böl(\" bir  iki \"), birleştir([1, 2, 3], \"-\"))")
                .unwrap(),
            "[\"a\", \"b\", \"c\"] [\"bir\", \"iki\"] 1-2-3\n"
        );
        assert_eq!(
            run("yaz(değiştir(\"merhaba dünya\", \"dünya\", \"Earth\"), ters(\"abc\"))").unwrap(),
            "merhaba Earth cba\n"
        );
    }

    #[test]
    fn provides_math_functions() {
        assert_eq!(
            run("yaz(mutlak(-3), karekök(16), üs(2, 10), yuvarla(2.567, 2), taban(2.7), tavan(2.1))")
                .unwrap(),
            "3 4 1024 2.57 2 3\n"
        );
        assert_eq!(
            run("yaz(en_küçük(3, 1, 2), en_büyük([3, 9, 2]), topla([1, 2, 3.5]))").unwrap(),
            "1 9 6.5\n"
        );
        let source = "\
her i içinde aralık(100) {
    değişken r = rastgele(1, 6)
    eğer r < 1 veya r > 6 veya r != taban(r) { yaz(\"hata\", r) }
}
değişken k = rastgele()
yaz(k >= 0 ve k < 1)";
        assert_eq!(run(source).unwrap(), "doğru\n");
    }

    #[test]
    fn provides_list_functions() {
        assert_eq!(
            run("değişken l = [3, 1, 2]\nyaz(sırala(l), l, ters(l), dilim(l, 1), dilim(l, 0, -1))")
                .unwrap(),
            "[1, 2, 3] [3, 1, 2] [2, 1, 3] [1, 2] [3, 1]\n"
        );
        assert_eq!(
            run("yaz(sırala([\"b\", \"a\"]), dilim(\"Merhaba\", 0, 3))").unwrap(),
            "[\"a\", \"b\"] Mer\n"
        );
        assert_eq!(
            run_error("sırala([1, \"a\"])").text(Lang::Tr),
            "Yalnızca hepsi sayı ya da hepsi metin olan listeler sıralanabilir"
        );
        assert_eq!(
            run_error("print(upper(1))").text(Lang::En),
            "'upper' expects string, got number"
        );
    }

    #[test]
    fn catches_thrown_values_and_runtime_errors() {
        let source = "\
fonksiyon böl_güvenli(a, b) {
    eğer b == 0 { fırlat {\"kod\": 1, \"mesaj\": \"sıfır\"} }
    döndür a / b
}
dene {
    yaz(böl_güvenli(1, 0))
    yaz(\"buraya gelinmez\")
} yakala h {
    yaz(\"yakalandı:\", h[\"mesaj\"])
}
dene { yaz([1][5]) } yakala h { yaz(h) }
dene { fırlat \"x\" } yakala { yaz(\"değişkensiz\") }
yaz(\"devam\")";
        assert_eq!(
            run(source).unwrap(),
            "yakalandı: sıfır\nİndeks 5 sınırların dışında (uzunluk 1)\ndeğişkensiz\ndevam\n"
        );
    }

    #[test]
    fn catches_in_english_and_reports_uncaught_errors() {
        assert_eq!(
            run("try {\n  print(1 / 0)\n}\ncatch e {\n  print(\"caught:\", e)\n}").unwrap(),
            "caught: Division by zero\n"
        );
        let error = run_error("throw [1, true]");
        assert_eq!(error.render(Lang::En), "1:1: Uncaught error: [1, true]");
        assert_eq!(error.text(Lang::Tr), "Yakalanmamış hata: [1, doğru]");
    }

    #[test]
    fn lets_control_flow_pass_through_try() {
        let source = "\
fonksiyon ilk_çift(liste) {
    her x içinde liste {
        dene {
            eğer x % 2 == 0 { döndür x }
        } yakala { }
    }
}
yaz(ilk_çift([1, 3, 4, 5]))";
        assert_eq!(run(source).unwrap(), "4\n");
    }

    fn run_in(directory: &std::path::Path, source: &str) -> Result<String, Error> {
        let mut output = Vec::new();
        let mut input = std::io::empty();
        let program = super::parse(source)?;
        let mut interpreter = super::Interpreter::new(&mut output, &mut input, Lang::Tr);
        interpreter.set_base_dir(directory);
        interpreter.run(&program)?;
        drop(interpreter);
        Ok(String::from_utf8(output).unwrap())
    }

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let directory =
            std::env::temp_dir().join(format!("earth-test-{name}-{}", std::process::id()));
        std::fs::create_dir_all(directory.join("alt")).unwrap();
        directory
    }

    #[test]
    fn imports_modules_once_relative_to_the_importer() {
        let directory = temp_dir("import");
        std::fs::write(
            directory.join("geometri.earth"),
            "içe_aktar \"alt/sabitler.earth\"\nyaz(\"geometri yüklendi\")\nfonksiyon alan(r) { döndür PI * r * r }",
        )
        .unwrap();
        std::fs::write(directory.join("alt/sabitler.earth"), "değişken PI = 3").unwrap();

        let output = run_in(
            &directory,
            "içe_aktar \"geometri.earth\"\niçe_aktar \"geometri.earth\"\nyaz(alan(2), PI)",
        )
        .unwrap();
        assert_eq!(output, "geometri yüklendi\n12 3\n");
        // A module cannot see the importer's variables.
        std::fs::write(directory.join("alt/gizli.earth"), "yaz(gizli)").unwrap();
        let error = run_in(
            &directory,
            "değişken gizli = 1\niçe_aktar \"alt/gizli.earth\"",
        )
        .unwrap_err();
        assert!(error
            .text(Lang::Tr)
            .starts_with("Tanımsız değişken 'gizli'"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn reports_import_errors_in_the_module_file() {
        let directory = temp_dir("import-errors");
        std::fs::write(directory.join("a.earth"), "içe_aktar \"b.earth\"").unwrap();
        std::fs::write(directory.join("b.earth"), "içe_aktar \"a.earth\"").unwrap();
        std::fs::write(
            directory.join("bozuk.earth"),
            "fonksiyon f() {\n  döndür 1 / 0\n}",
        )
        .unwrap();

        let error = run_in(&directory, "içe_aktar \"a.earth\"").unwrap_err();
        assert_eq!(
            error.text(Lang::Tr),
            "'a.earth' kendini dolaylı olarak içe aktarıyor (döngüsel içe aktarma)"
        );
        assert_eq!(error.file, Some(directory.join("b.earth")));

        let error = run_in(&directory, "içe_aktar \"bozuk.earth\"\nf()").unwrap_err();
        assert_eq!(error.file, Some(directory.join("bozuk.earth")));
        assert_eq!(error.position, Some(Position::new(2, 12)));

        let error = run_in(&directory, "içe_aktar \"yok.earth\"").unwrap_err();
        assert!(error
            .text(Lang::Tr)
            .starts_with("'yok.earth' içe aktarılamadı"));
        std::fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn stops_when_interrupted_even_inside_dene() {
        use std::sync::atomic::{AtomicBool, Ordering};
        use std::sync::Arc;

        let flag = Arc::new(AtomicBool::new(false));
        let setter = Arc::clone(&flag);
        let stopper = std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(50));
            setter.store(true, Ordering::Relaxed);
        });

        let mut output = Vec::new();
        let mut input = std::io::empty();
        let program =
            super::parse("dene {\n  iken doğru { }\n} yakala { yaz(\"yakalandı\") }").unwrap();
        let mut interpreter = super::Interpreter::new(&mut output, &mut input, Lang::Tr);
        interpreter.set_interrupt(Arc::clone(&flag));
        let error = interpreter.run(&program).unwrap_err();
        drop(interpreter);
        stopper.join().unwrap();

        assert_eq!(error.text(Lang::Tr), "Program durduruldu");
        assert!(output.is_empty());
    }
}
