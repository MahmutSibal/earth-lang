use std::io::{self, BufRead, Write};
use std::{env, fs, process, thread};

use earth::{Error, Interpreter};

/// The interpreter recurses on the native stack; give deep Earth recursion
/// (up to `MAX_CALL_DEPTH`) enough room, even in debug builds.
const STACK_SIZE: usize = 256 * 1024 * 1024;

fn main() {
    let code = thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(start)
        .expect("could not start the interpreter thread")
        .join()
        .unwrap_or(101);
    process::exit(code);
}

fn start() -> i32 {
    let mut arguments = env::args().skip(1);
    match (arguments.next(), arguments.next()) {
        (None, _) => repl(),
        (Some(flag), None) if flag == "-h" || flag == "--help" || flag == "--yardım" => {
            println!("Kullanım: earth [kaynak-dosyası.earth]");
            println!("Dosya verilmezse etkileşimli kabuk (REPL) açılır.");
            0
        }
        (Some(path), None) => run_file(&path),
        _ => {
            eprintln!("Kullanım: earth [kaynak-dosyası.earth]");
            2
        }
    }
}

fn run_file(path: &str) -> i32 {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("'{path}' okunamadı: {error}");
            return 1;
        }
    };

    let stdout = io::stdout();
    let stdin = io::stdin();
    let result = earth::run_with_io(&source, &mut stdout.lock(), &mut stdin.lock());
    match result {
        Ok(()) => 0,
        Err(error) => {
            report(path, &source, &error);
            1
        }
    }
}

/// Prints an error with the offending source line and a caret under it.
fn report(path: &str, source: &str, error: &Error) {
    let Some(position) = error.position else {
        eprintln!("Earth hatası: {}", error.message);
        return;
    };
    eprintln!("Earth hatası: {path}:{position}: {}", error.message);
    if let Some(line) = source.lines().nth(position.line - 1) {
        let gutter = position.line.to_string();
        let padding: String = line
            .chars()
            .take(position.column - 1)
            .map(|c| if c == '\t' { '\t' } else { ' ' })
            .collect();
        eprintln!("{} |", " ".repeat(gutter.len()));
        eprintln!("{gutter} | {line}");
        eprintln!("{} | {padding}^", " ".repeat(gutter.len()));
    }
}

fn repl() -> i32 {
    println!("Earth {} — çıkmak için Ctrl+D", env!("CARGO_PKG_VERSION"));

    let mut stdout = io::stdout();
    let mut stdin = io::stdin().lock();
    // `girdi` inside the REPL reads from the same terminal.
    let mut program_input = io::BufReader::new(io::stdin());
    let mut interpreter = Interpreter::new(&mut stdout, &mut program_input);

    let mut buffer = String::new();
    loop {
        let prompt = if buffer.is_empty() {
            "earth> "
        } else {
            "...    "
        };
        print!("{prompt}");
        let _ = io::stdout().flush();

        let mut line = String::new();
        match stdin.read_line(&mut line) {
            Ok(0) => {
                println!();
                return 0;
            }
            Ok(_) => buffer.push_str(&line),
            Err(error) => {
                eprintln!("Okuma hatası: {error}");
                return 1;
            }
        }

        if is_incomplete(&buffer) {
            continue;
        }

        match earth::parse(&buffer).and_then(|program| interpreter.run(&program)) {
            Ok(Some(value)) if !matches!(value, earth::value::Value::Nil) => {
                println!("{}", repr(&value));
            }
            Ok(_) => {}
            Err(error) => eprintln!("Hata: {error}"),
        }
        buffer.clear();
    }
}

fn repr(value: &earth::value::Value) -> String {
    match value {
        earth::value::Value::String(text) => format!("{text:?}"),
        other => other.to_string(),
    }
}

/// Keeps reading while braces, brackets or parentheses are still open, so
/// that blocks can be typed over several lines.
fn is_incomplete(source: &str) -> bool {
    let mut depth: i32 = 0;
    let mut in_string = false;
    let mut escaped = false;
    for line in source.lines() {
        for character in line.chars() {
            if in_string {
                match character {
                    _ if escaped => escaped = false,
                    '\\' => escaped = true,
                    '"' => in_string = false,
                    _ => {}
                }
                continue;
            }
            match character {
                '#' => break,
                '"' => in_string = true,
                '{' | '(' | '[' => depth += 1,
                '}' | ')' | ']' => depth -= 1,
                _ => {}
            }
        }
    }
    depth > 0 || in_string
}
