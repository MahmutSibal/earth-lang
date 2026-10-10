//! Runs every program in `examples/` and checks its output.

use std::fs;

fn run_example(name: &str, input: &str) -> String {
    let source = fs::read_to_string(format!("examples/{name}")).unwrap();
    let mut output = Vec::new();
    let lang = earth::detect_language(&source);
    let program = earth::parse(&source).unwrap();
    let mut input = input.as_bytes();
    let mut interpreter = earth::Interpreter::new(&mut output, &mut input, lang);
    interpreter.set_base_dir("examples");
    interpreter.run(&program).unwrap();
    drop(interpreter);
    String::from_utf8(output).unwrap()
}

#[test]
fn hello() {
    assert_eq!(
        run_example("hello.earth", ""),
        "Merhaba, Earth!\nHello, Earth!\n"
    );
}

#[test]
fn fizzbuzz() {
    let output = run_example("fizzbuzz.earth", "");
    let lines: Vec<&str> = output.lines().collect();
    assert_eq!(lines.len(), 15);
    assert_eq!(lines[2], "Fizz");
    assert_eq!(lines[4], "Buzz");
    assert_eq!(lines[14], "FizzBuzz");
}

#[test]
fn fibonacci() {
    assert_eq!(
        run_example("fibonacci.earth", ""),
        "First ten: [0, 1, 1, 2, 3, 5, 8, 13, 21, 34]\n"
    );
}

#[test]
fn liste() {
    assert_eq!(
        run_example("liste.earth", ""),
        "Ortalama: 78.25\n72 → geçti\n95 → geçti\n58 → kaldı\n88 → geçti\n"
    );
}

#[test]
fn selam() {
    assert_eq!(
        run_example("selam.earth", "Ada\n"),
        "Adın ne? Merhaba, Ada!\n"
    );
    assert_eq!(run_example("selam.earth", ""), "Adın ne? Merhaba, Dünya!\n");
}

#[test]
fn sozluk() {
    assert_eq!(run_example("sozluk.earth", ""), "bir: 3\niki: 2\nüç: 1\n");
}

#[test]
fn araclar_only_defines_functions() {
    assert_eq!(run_example("araclar.earth", ""), "");
}

#[test]
fn moduller() {
    assert_eq!(
        run_example("moduller.earth", ""),
        "Merhaba, Ada!\n\
         Ortalama: 81.66666666666667\n\
         Hata yakalandı: Boş listenin ortalaması olmaz\n\
         armut, elma, kiraz\n\
         İSTANBUL\n"
    );
}

#[test]
fn every_example_is_covered() {
    let mut names: Vec<String> = fs::read_dir("examples")
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "araclar.earth",
            "fibonacci.earth",
            "fizzbuzz.earth",
            "hello.earth",
            "liste.earth",
            "moduller.earth",
            "selam.earth",
            "sozluk.earth"
        ]
    );
}
