//! The debugger hooks: where a program pauses and what it shows there.

use std::io;
use std::path::Path;

use earth::debug::{Debugger, Snapshot};
use earth::{Interpreter, Lang};

/// Pauses on the given lines and records what it saw.
struct Recorder {
    lines: Vec<usize>,
    seen: Vec<Snapshot>,
    stop_after: Option<usize>,
}

impl Debugger for Recorder {
    fn should_pause(&mut self, line: usize, _file: Option<&Path>, _depth: usize) -> bool {
        self.lines.contains(&line)
    }

    fn pause(&mut self, snapshot: Snapshot) -> bool {
        self.seen.push(snapshot);
        self.stop_after != Some(self.seen.len())
    }
}

fn run(source: &str, recorder: &mut Recorder) -> Result<String, earth::Error> {
    let program = earth::parse(source)?;
    let mut output = Vec::new();
    let mut input = io::empty();
    let result = {
        let mut interpreter = Interpreter::new(&mut output, &mut input, Lang::Tr);
        interpreter.set_debugger(recorder);
        interpreter.run(&program).map(|_| ())
    };
    result.map(|()| String::from_utf8(output).unwrap())
}

const PROGRAM: &str = "\
değişken toplam = 0
fonksiyon kare(x) {
    döndür x * x
}
her i içinde aralık(1, 4) {
    toplam = toplam + kare(i)
}
yaz(toplam)
";

#[test]
fn shows_variables_calls_and_loop_turns() {
    let mut recorder = Recorder {
        lines: vec![3],
        seen: Vec::new(),
        stop_after: None,
    };
    let output = run(PROGRAM, &mut recorder).unwrap();
    assert_eq!(output, "14\n");
    assert_eq!(recorder.seen.len(), 3, "once per call");

    let second = &recorder.seen[1];
    assert_eq!(second.line, 3);
    let frames: Vec<(&str, usize)> = second
        .frames
        .iter()
        .map(|frame| (frame.function.as_str(), frame.line))
        .collect();
    assert_eq!(frames, vec![("", 6), ("kare", 3)]);
    assert_eq!(second.loops.len(), 1);
    assert_eq!(second.loops[0].line, 5);
    assert_eq!(second.loops[0].turn, 2);
    assert_eq!(second.loops[0].total, Some(3));

    let x = second.variables.iter().find(|v| v.name == "x").unwrap();
    assert_eq!(
        (x.value.as_str(), x.type_name.as_str(), x.global),
        ("2", "sayı", false)
    );
    let toplam = second
        .variables
        .iter()
        .find(|v| v.name == "toplam")
        .unwrap();
    assert_eq!((toplam.value.as_str(), toplam.global), ("1", true));
    // Built-in functions are not variables.
    assert!(second.variables.iter().all(|v| v.name != "yaz"));
}

#[test]
fn counts_while_turns_and_quotes_text() {
    let mut recorder = Recorder {
        lines: vec![4],
        seen: Vec::new(),
        stop_after: None,
    };
    let source = "değişken i = 0\ndeğişken ad = \"Ada\"\niken i < 3 {\n    i = i + 1\n}\n";
    run(source, &mut recorder).unwrap();
    let turns: Vec<usize> = recorder.seen.iter().map(|s| s.loops[0].turn).collect();
    assert_eq!(turns, vec![1, 2, 3]);
    assert_eq!(recorder.seen[0].loops[0].total, None);
    let ad = recorder.seen[0]
        .variables
        .iter()
        .find(|v| v.name == "ad")
        .unwrap();
    assert_eq!(ad.value, "\"Ada\"");
}

#[test]
fn pauses_on_top_level_expressions_and_keeps_the_call_line() {
    let mut recorder = Recorder {
        lines: vec![3, 8],
        seen: Vec::new(),
        stop_after: None,
    };
    // Line 3 holds no statement; line 8 is a top-level call.
    let source = "fonksiyon kare(x) {\n    döndür x * x\n}\n\n\n\n\nyaz(kare(3))\n";
    run(source, &mut recorder).unwrap();
    assert_eq!(recorder.seen.len(), 1);
    assert_eq!(recorder.seen[0].line, 8);

    let mut inside = Recorder {
        lines: vec![2],
        seen: Vec::new(),
        stop_after: None,
    };
    run(source, &mut inside).unwrap();
    let frames: Vec<(&str, usize)> = inside.seen[0]
        .frames
        .iter()
        .map(|frame| (frame.function.as_str(), frame.line))
        .collect();
    assert_eq!(frames, vec![("", 8), ("kare", 2)]);
}

#[test]
fn stopping_at_a_pause_ends_the_program() {
    let mut recorder = Recorder {
        lines: vec![6],
        seen: Vec::new(),
        stop_after: Some(1),
    };
    let error = run(PROGRAM, &mut recorder).unwrap_err();
    assert_eq!(error.message, earth::messages::Message::Interrupted);
}

#[test]
fn no_debugger_means_no_pauses() {
    // The same program runs normally without one.
    assert_eq!(earth::run(PROGRAM).unwrap(), "14\n");
}
