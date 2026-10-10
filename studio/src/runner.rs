//! Runs Earth programs on a background thread so the window stays
//! responsive. Output streams back as events; `girdi` reads lines typed into
//! the terminal; the Stop button sets the interpreter's interrupt flag.

use std::io::{self, BufRead, Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use earth::value::Value;
use earth::{Interpreter, Lang};

/// The interpreter recurses on the native stack, like the command-line
/// `earth`, so the worker gets the same generous stack.
const STACK_SIZE: usize = 256 * 1024 * 1024;

enum Command {
    Run {
        source: String,
        base_dir: PathBuf,
        file: Option<PathBuf>,
    },
    Repl {
        code: String,
        lang: Lang,
    },
}

pub enum Event {
    Output(String),
    /// The program asked for input with `girdi`.
    WaitingForInput,
    Error {
        text: String,
        line: Option<usize>,
        column: Option<usize>,
        /// The file the error happened in, when known.
        file: Option<PathBuf>,
    },
    /// A REPL expression's value.
    Value(String),
    Finished {
        interrupted: bool,
    },
}

pub struct Runner {
    commands: Sender<Command>,
    events: Receiver<Event>,
    input: Sender<String>,
    interrupt: Arc<AtomicBool>,
    busy: bool,
}

impl Runner {
    /// `wake` is called whenever an event is ready, to repaint the window.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (commands, command_receiver) = mpsc::channel();
        let (event_sender, events) = mpsc::channel();
        let (input, input_receiver) = mpsc::channel();
        let interrupt = Arc::new(AtomicBool::new(false));

        let worker = Worker {
            events: event_sender,
            input: Arc::new(Mutex::new(input_receiver)),
            interrupt: Arc::clone(&interrupt),
            wake: Arc::new(wake),
        };
        thread::Builder::new()
            .name("earth-runner".to_owned())
            .stack_size(STACK_SIZE)
            .spawn(move || worker.serve(command_receiver))
            .expect("could not start the Earth runner thread");

        Self {
            commands,
            events,
            input,
            interrupt,
            busy: false,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    pub fn run(&mut self, source: String, base_dir: PathBuf, file: Option<PathBuf>) {
        self.start(Command::Run {
            source,
            base_dir,
            file,
        });
    }

    pub fn repl(&mut self, code: String, lang: Lang) {
        self.start(Command::Repl { code, lang });
    }

    fn start(&mut self, command: Command) {
        self.interrupt.store(false, Ordering::Relaxed);
        self.busy = self.commands.send(command).is_ok();
    }

    pub fn send_input(&self, line: String) {
        let _ = self.input.send(line);
    }

    pub fn stop(&self) {
        self.interrupt.store(true, Ordering::Relaxed);
    }

    /// Events that arrived since the last call.
    pub fn poll(&mut self) -> Vec<Event> {
        let events: Vec<Event> = self.events.try_iter().collect();
        if events
            .iter()
            .any(|event| matches!(event, Event::Finished { .. }))
        {
            self.busy = false;
        }
        events
    }
}

struct Worker {
    events: Sender<Event>,
    input: Arc<Mutex<Receiver<String>>>,
    interrupt: Arc<AtomicBool>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Worker {
    fn serve(self, commands: Receiver<Command>) {
        // The REPL keeps its variables between commands.
        let mut repl_output = self.output();
        let mut repl_input = self.input();
        let mut repl: Option<(Lang, Interpreter)> = None;

        for command in commands {
            match command {
                Command::Run {
                    source,
                    base_dir,
                    file,
                } => self.run(&source, base_dir, file),
                Command::Repl { code, lang } => {
                    if repl.as_ref().is_none_or(|(current, _)| *current != lang) {
                        let mut interpreter =
                            Interpreter::new(&mut repl_output, &mut repl_input, lang);
                        interpreter.set_interrupt(Arc::clone(&self.interrupt));
                        repl = Some((lang, interpreter));
                    }
                    if let Some((lang, interpreter)) = repl.as_mut() {
                        let result =
                            earth::parse(&code).and_then(|program| interpreter.run(&program));
                        match result {
                            Ok(Some(value)) if !matches!(value, Value::Nil) => {
                                self.send(Event::Value(repr(&value, *lang)));
                            }
                            Ok(_) => {}
                            Err(error) => self.report(&error, *lang, None),
                        }
                    }
                }
            }
            // Input the program never read must not leak into the next run.
            if let Ok(lines) = self.input.lock() {
                while lines.try_recv().is_ok() {}
            }
            let interrupted = self.interrupt.load(Ordering::Relaxed);
            self.send(Event::Finished { interrupted });
        }
    }

    fn run(&self, source: &str, base_dir: PathBuf, file: Option<PathBuf>) {
        let lang = earth::detect_language(source);
        let mut output = self.output();
        let mut input = self.input();
        let result = earth::parse(source).and_then(|program| {
            let mut interpreter = Interpreter::new(&mut output, &mut input, lang);
            interpreter.set_base_dir(base_dir);
            interpreter.set_interrupt(Arc::clone(&self.interrupt));
            interpreter.run(&program)
        });
        if let Err(error) = result {
            if error.message != earth::messages::Message::Interrupted {
                self.report(&error, lang, file);
            }
        }
    }

    fn report(&self, error: &earth::Error, lang: Lang, main_file: Option<PathBuf>) {
        self.send(Event::Error {
            text: error.text(lang),
            line: error.position.map(|position| position.line),
            column: error.position.map(|position| position.column),
            file: error.file.clone().or(main_file),
        });
    }

    fn output(&self) -> EventWriter {
        EventWriter {
            events: self.events.clone(),
            wake: Arc::clone(&self.wake),
        }
    }

    fn input(&self) -> TerminalInput {
        TerminalInput {
            lines: Arc::clone(&self.input),
            interrupt: Arc::clone(&self.interrupt),
            events: self.events.clone(),
            wake: Arc::clone(&self.wake),
            pending: Vec::new(),
        }
    }

    fn send(&self, event: Event) {
        let _ = self.events.send(event);
        (self.wake)();
    }
}

fn repr(value: &Value, lang: Lang) -> String {
    match value {
        Value::String(text) => format!("{text:?}"),
        other => other.display(lang).to_string(),
    }
}

/// Program output, sent to the terminal as it is written.
struct EventWriter {
    events: Sender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Write for EventWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let text = String::from_utf8_lossy(bytes).into_owned();
        self.events
            .send(Event::Output(text))
            .map_err(|_| io::Error::from(io::ErrorKind::BrokenPipe))?;
        (self.wake)();
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// `girdi` reads lines typed into the terminal. While waiting it keeps an
/// eye on the Stop button, so a stopped program does not hang on input.
struct TerminalInput {
    lines: Arc<Mutex<Receiver<String>>>,
    interrupt: Arc<AtomicBool>,
    events: Sender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
    /// Bytes of the current line not yet handed to the interpreter.
    pending: Vec<u8>,
}

impl Read for TerminalInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let available = self.fill_buf()?;
        let count = available.len().min(buffer.len());
        buffer[..count].copy_from_slice(&available[..count]);
        self.consume(count);
        Ok(count)
    }
}

impl BufRead for TerminalInput {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        if self.pending.is_empty() {
            let _ = self.events.send(Event::WaitingForInput);
            (self.wake)();
            let lines = self
                .lines
                .lock()
                .map_err(|_| io::Error::other("input lock poisoned"))?;
            loop {
                if self.interrupt.load(Ordering::Relaxed) {
                    // End of input; the interpreter then notices the stop.
                    return Ok(&[]);
                }
                match lines.recv_timeout(Duration::from_millis(50)) {
                    Ok(line) => {
                        self.pending = format!("{line}\n").into_bytes();
                        break;
                    }
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => return Ok(&[]),
                }
            }
        }
        Ok(&self.pending)
    }

    fn consume(&mut self, amount: usize) {
        self.pending.drain(..amount.min(self.pending.len()));
    }
}

#[cfg(test)]
mod tests {
    use super::{Event, Runner};
    use std::time::{Duration, Instant};

    fn wait_until_finished(runner: &mut Runner) -> Vec<Event> {
        let deadline = Instant::now() + Duration::from_secs(10);
        let mut events = Vec::new();
        while runner.is_busy() {
            assert!(Instant::now() < deadline, "the runner did not finish");
            events.extend(runner.poll());
            std::thread::sleep(Duration::from_millis(5));
        }
        events
    }

    fn output(events: &[Event]) -> String {
        events
            .iter()
            .filter_map(|event| match event {
                Event::Output(text) | Event::Value(text) => Some(text.as_str()),
                Event::Error { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn runs_programs_with_input_and_reports_errors() {
        let mut runner = Runner::new(|| {});
        runner.run(
            "değişken ad = girdi(\"? \")\nyaz(\"Merhaba \" + ad)\nyaz(1 / 0)".to_owned(),
            ".".into(),
            None,
        );
        runner.send_input("Ada".to_owned());
        let events = wait_until_finished(&mut runner);
        assert_eq!(output(&events), "? Merhaba Ada\nSıfıra bölme");
        assert!(events
            .iter()
            .any(|event| matches!(event, Event::Error { line: Some(3), .. })));
    }

    #[test]
    fn stops_endless_programs_and_waiting_input() {
        let mut runner = Runner::new(|| {});
        runner.run("iken doğru { }".to_owned(), ".".into(), None);
        std::thread::sleep(Duration::from_millis(50));
        runner.stop();
        let events = wait_until_finished(&mut runner);
        assert!(events
            .iter()
            .any(|event| matches!(event, Event::Finished { interrupted: true })));

        runner.run("girdi()".to_owned(), ".".into(), None);
        std::thread::sleep(Duration::from_millis(50));
        runner.stop();
        wait_until_finished(&mut runner);
    }

    #[test]
    fn keeps_repl_variables() {
        let mut runner = Runner::new(|| {});
        runner.repl("değişken x = 40".to_owned(), earth::Lang::Tr);
        wait_until_finished(&mut runner);
        runner.repl("x + 2".to_owned(), earth::Lang::Tr);
        let events = wait_until_finished(&mut runner);
        assert_eq!(output(&events), "42");
    }
}
