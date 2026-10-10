pub mod ast;
pub mod interpreter;
pub mod lexer;
pub mod parser;

pub fn run(source: &str) -> Result<Vec<String>, String> {
    let tokens = lexer::lex(source)?;
    let program = parser::parse(&tokens)?;
    interpreter::interpret(&program)
}

#[cfg(test)]
mod tests {
    use super::run;

    #[test]
    fn runs_turkish_print_alias() {
        assert_eq!(
            run("yaz(\"Merhaba, Earth!\")").unwrap(),
            vec!["Merhaba, Earth!"]
        );
    }

    #[test]
    fn runs_english_print_alias() {
        assert_eq!(
            run("print(\"Hello, Earth!\")").unwrap(),
            vec!["Hello, Earth!"]
        );
    }

    #[test]
    fn reports_malformed_input() {
        assert_eq!(
            run("print(\"missing\""),
            Err("Expected ')' after quoted string".to_owned())
        );
    }
}
