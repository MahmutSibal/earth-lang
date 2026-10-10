use std::io::{self, BufRead, Write};
use std::path::Path;
use std::{env, fs, process, thread};

use earth::messages::cli;
use earth::value::Value;
use earth::{Error, Interpreter, Lang};

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

struct Options {
    lang: Option<Lang>,
    path: Option<String>,
}

fn start() -> i32 {
    let options = match parse_arguments(env::args().skip(1)) {
        Ok(Some(options)) => options,
        Ok(None) => return 0,
        Err(code) => return code,
    };
    match options.path {
        Some(path) => {
            let code = run_file(&path, options.lang);
            if owns_console() {
                // Opened by double-clicking a file: keep the window open so
                // the output can be read.
                println!();
                print!("{}", cli::press_enter(options.lang.unwrap_or_default()));
                let _ = io::stdout().flush();
                let _ = io::stdin().read_line(&mut String::new());
            }
            code
        }
        None => repl(options.lang.unwrap_or_default()),
    }
}

/// True when Earth was started from Explorer rather than a terminal, so the
/// console window closes as soon as Earth exits.
#[cfg(windows)]
fn owns_console() -> bool {
    #[link(name = "kernel32")]
    extern "system" {
        fn GetConsoleProcessList(process_list: *mut u32, count: u32) -> u32;
    }
    let mut processes = [0u32; 2];
    // SAFETY: the buffer is valid for `processes.len()` entries.
    let attached = unsafe { GetConsoleProcessList(processes.as_mut_ptr(), processes.len() as u32) };
    attached == 1
}

#[cfg(not(windows))]
fn owns_console() -> bool {
    false
}

/// Returns `Ok(None)` when the program should exit successfully, such as
/// after printing help.
fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Option<Options>, i32> {
    let mut options = Options {
        lang: None,
        path: None,
    };
    let mut arguments = arguments.peekable();
    while let Some(argument) = arguments.next() {
        // Messages before the language is known use the one asked for so
        // far, or Turkish.
        let lang = options.lang.unwrap_or_default();
        match argument.as_str() {
            "-h" | "--help" | "--yardım" | "--yardim" => {
                println!("{}", cli::usage(lang));
                println!("{}", cli::help(lang));
                return Ok(None);
            }
            "-v" | "--version" | "--sürüm" | "--surum" => {
                println!("Earth {}", env!("CARGO_PKG_VERSION"));
                return Ok(None);
            }
            "--dil" | "--lang" => {
                let Some(code) = arguments.next() else {
                    eprintln!("{}", cli::usage(lang));
                    return Err(2);
                };
                match Lang::from_code(&code) {
                    Some(chosen) => options.lang = Some(chosen),
                    None => {
                        eprintln!("{}", cli::unknown_language(lang, &code));
                        return Err(2);
                    }
                }
            }
            _ if options.path.is_none() && !argument.starts_with('-') => {
                options.path = Some(argument);
            }
            _ => {
                eprintln!("{}", cli::usage(lang));
                return Err(2);
            }
        }
    }
    Ok(Some(options))
}

fn run_file(path: &str, lang: Option<Lang>) -> i32 {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) => {
            let lang = lang.unwrap_or_default();
            eprintln!("{}", cli::cannot_read(lang, path, &error));
            return 1;
        }
    };
    let lang = lang.unwrap_or_else(|| earth::detect_language(&source));

    let mut stdout = io::stdout().lock();
    let mut stdin = io::stdin().lock();
    let result = earth::parse(&source).and_then(|program| {
        let mut interpreter = Interpreter::new(&mut stdout, &mut stdin, lang);
        if let Some(directory) = Path::new(path).parent() {
            interpreter.set_base_dir(directory);
        }
        interpreter.run(&program)
    });
    match result {
        Ok(_) => 0,
        Err(error) => {
            report(path, &source, &error, lang);
            1
        }
    }
}

/// Prints an error with the offending source line and a caret under it.
/// Errors inside an imported module point into that module's file.
fn report(path: &str, source: &str, error: &Error, lang: Lang) {
    if let Some(file) = &error.file {
        let module_source = fs::read_to_string(file).unwrap_or_default();
        let module_error = Error {
            file: None,
            ..error.clone()
        };
        return report(
            &file.display().to_string(),
            &module_source,
            &module_error,
            lang,
        );
    }
    let prefix = cli::error_prefix(lang);
    let Some(position) = error.position else {
        eprintln!("{prefix}: {}", error.text(lang));
        return;
    };
    eprintln!("{prefix}: {path}:{position}: {}", error.text(lang));
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

fn repl(lang: Lang) -> i32 {
    println!("{}", cli::repl_banner(lang, env!("CARGO_PKG_VERSION")));

    let mut stdout = io::stdout();
    let mut stdin = io::stdin().lock();
    // `girdi` inside the REPL reads from the same terminal.
    let mut program_input = io::BufReader::new(io::stdin());
    let mut interpreter = Interpreter::new(&mut stdout, &mut program_input, lang);

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
                eprintln!("{}: {error}", cli::repl_error(lang));
                return 1;
            }
        }

        if buffer.trim().is_empty() {
            buffer.clear();
            continue;
        }
        if matches!(buffer.trim(), "çık" | "cik" | "exit" | "quit") {
            return 0;
        }
        if is_incomplete(&buffer) {
            continue;
        }

        match earth::parse(&buffer).and_then(|program| interpreter.run(&program)) {
            Ok(Some(value)) if !matches!(value, Value::Nil) => {
                println!("{}", repr(&value, interpreter.lang()));
            }
            Ok(_) => {}
            Err(error) => eprintln!("{}: {}", cli::repl_error(lang), error.render(lang)),
        }
        buffer.clear();
    }
}

fn repr(value: &Value, lang: Lang) -> String {
    match value {
        Value::String(text) => format!("{text:?}"),
        other => other.display(lang).to_string(),
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
