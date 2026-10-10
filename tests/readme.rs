//! Keeps the README honest: every ```earth block must parse, and a block
//! followed directly by a ```text block must print exactly that text.

use std::fs;

struct Example {
    line: usize,
    source: String,
    expected: Option<String>,
}

fn examples() -> Vec<Example> {
    let readme = fs::read_to_string("README.md").unwrap();
    let lines: Vec<&str> = readme.lines().collect();
    let mut examples = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        if lines[index].trim() != "```earth" {
            index += 1;
            continue;
        }
        let start = index + 1;
        let (source, after) = block(&lines, start);
        // An output block may follow after blank lines only.
        let mut next = after;
        while next < lines.len() && lines[next].trim().is_empty() {
            next += 1;
        }
        let expected = (next < lines.len() && lines[next].trim() == "```text").then(|| {
            let (output, _) = block(&lines, next + 1);
            output
        });
        examples.push(Example {
            line: start,
            source,
            expected,
        });
        index = after;
    }
    examples
}

/// The text of a fenced block starting at `start`, and the line after it.
/// Indentation from a surrounding list item is removed.
fn block(lines: &[&str], start: usize) -> (String, usize) {
    let indent = lines[start - 1].len() - lines[start - 1].trim_start().len();
    let mut end = start;
    while lines[end].trim() != "```" {
        end += 1;
    }
    let text: Vec<&str> = lines[start..end]
        .iter()
        .map(|line| line.get(indent..).unwrap_or("").trim_end())
        .collect();
    (format!("{}\n", text.join("\n")), end + 1)
}

#[test]
fn readme_examples_parse_and_print_what_they_promise() {
    let examples = examples();
    assert!(
        examples.len() > 20,
        "found only {} examples",
        examples.len()
    );
    let mut checked = 0;
    for example in &examples {
        if let Err(error) = earth::parse(&example.source) {
            panic!("README line {}: does not parse: {error}", example.line);
        }
        if let Some(expected) = &example.expected {
            let output = earth::run(&example.source)
                .unwrap_or_else(|error| panic!("README line {}: {error}", example.line));
            assert_eq!(
                &output, expected,
                "README line {}: output differs",
                example.line
            );
            checked += 1;
        }
    }
    assert!(checked > 15, "only {checked} examples have outputs");
}
