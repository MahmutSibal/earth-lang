//! The `earth` command: running a file, reporting its errors, and the
//! interactive shell.

use std::io::Write;
use std::process::{Command, Output, Stdio};

fn earth(arguments: &[&str], input: &str) -> Output {
    let mut child = Command::new(env!("CARGO_BIN_EXE_earth"))
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(input.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn runs_a_file() {
    let output = earth(&["examples/fizzbuzz.earth"], "");
    assert!(output.status.success());
    assert!(text(&output.stdout).contains("FizzBuzz"));
}

#[test]
fn reports_an_error_with_its_line_and_fails() {
    let path = std::env::temp_dir().join("earth-cli-hatali.earth");
    std::fs::write(&path, "yaz(\"a\")\nyaz(1 +)\n").unwrap();
    let output = earth(&[path.to_str().unwrap()], "");
    assert_eq!(output.status.code(), Some(1));
    let errors = text(&output.stderr);
    assert!(errors.contains("Earth hatası"), "{errors}");
    assert!(errors.contains(":2:8:"), "{errors}");
    assert!(errors.contains("2 | yaz(1 +)"), "{errors}");
}

#[test]
fn check_validates_without_running_the_file() {
    let path = std::env::temp_dir().join("earth-cli-check.earth");
    std::fs::write(&path, "girdi(\"Bu çıktı yazılmamalı\")\n").unwrap();
    let output = earth(&["--check", path.to_str().unwrap()], "");
    assert!(output.status.success());
    assert!(text(&output.stdout).is_empty());
    assert!(text(&output.stderr).is_empty());
}

#[test]
fn check_reports_syntax_errors() {
    let path = std::env::temp_dir().join("earth-cli-check-hatali.earth");
    std::fs::write(&path, "yaz(1 +)\n").unwrap();
    let output = earth(&["--check", path.to_str().unwrap()], "");
    assert_eq!(output.status.code(), Some(1));
    let errors = text(&output.stderr);
    assert!(errors.contains("Earth hatası"), "{errors}");
    assert!(errors.contains(":1:8:"), "{errors}");
    assert!(errors.contains("1 | yaz(1 +)"), "{errors}");
}

#[test]
fn shell_keeps_definitions_and_prints_values() {
    let output = earth(
        &[],
        "fonksiyon kare(x) {\n    döndür x * x\n}\nkare(7)\nyaz(tanımsız)\nçık\n",
    );
    assert!(output.status.success());
    assert!(text(&output.stdout).contains("49"));
    assert!(text(&output.stderr).contains("tanımsız"));
}
