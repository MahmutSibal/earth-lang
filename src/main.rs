use std::{env, fs, process};

fn main() {
    let path = match env::args().nth(1) {
        Some(path) => path,
        None => {
            eprintln!("Usage: earth <source-file>");
            process::exit(2);
        }
    };

    let source = match fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) => {
            eprintln!("Could not read '{path}': {error}");
            process::exit(1);
        }
    };

    match earth::run(&source) {
        Ok(output) => {
            for line in output {
                println!("{line}");
            }
        }
        Err(error) => {
            eprintln!("Earth error: {error}");
            process::exit(1);
        }
    }
}
