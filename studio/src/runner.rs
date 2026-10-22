//! Runs Earth programs on a background thread so the window stays
//! responsive. Output streams back as events; `girdi` reads lines typed into
//! the terminal; the Stop button sets the interpreter's interrupt flag.
//! A program run for debugging pauses at breakpoints and steps, sending
//! what it sees as `Event::Paused` and waiting for a `Resume`.

use std::collections::BTreeSet;
use std::io::{self, BufRead, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use earth::debug::{Debugger, Snapshot};
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
        debug: bool,
    },
    Python {
        file: PathBuf,
        base_dir: PathBuf,
        interpreter: Option<String>,
    },
    Repl {
        code: String,
        lang: Lang,
    },
}

pub enum Event {
    Output(String),
    /// Output written to a Python process's standard error stream.
    ErrorOutput(String),
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
    /// A program run for debugging stopped at a breakpoint or a step.
    Paused(Snapshot),
}

/// How a paused program goes on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Resume {
    /// Until the next breakpoint.
    Continue,
    /// To the next statement, wherever it is.
    StepInto,
    /// To the next statement in this function or an outer one.
    StepOver,
    /// To the next statement after this function returns.
    StepOut,
}

pub struct Runner {
    commands: Sender<Command>,
    events: Receiver<Event>,
    input: Sender<String>,
    resume: Sender<Resume>,
    interrupt: Arc<AtomicBool>,
    /// Lines of the main program to pause on; changes apply at once, even
    /// while a program runs.
    breakpoints: Arc<Mutex<BTreeSet<usize>>>,
    child: Arc<Mutex<Option<Child>>>,
    busy: bool,
}

impl Runner {
    /// `wake` is called whenever an event is ready, to repaint the window.
    pub fn new(wake: impl Fn() + Send + Sync + 'static) -> Self {
        let (commands, command_receiver) = mpsc::channel();
        let (event_sender, events) = mpsc::channel();
        let (input, input_receiver) = mpsc::channel();
        let (resume, resume_receiver) = mpsc::channel();
        let interrupt = Arc::new(AtomicBool::new(false));
        let breakpoints = Arc::new(Mutex::new(BTreeSet::new()));
        let child = Arc::new(Mutex::new(None));

        let worker = Worker {
            events: event_sender,
            input: Arc::new(Mutex::new(input_receiver)),
            resume: Arc::new(Mutex::new(resume_receiver)),
            interrupt: Arc::clone(&interrupt),
            breakpoints: Arc::clone(&breakpoints),
            child: Arc::clone(&child),
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
            resume,
            interrupt,
            breakpoints,
            child,
            busy: false,
        }
    }

    pub fn is_busy(&self) -> bool {
        self.busy
    }

    /// Runs a program; with `debug` it pauses at breakpoints, or at its
    /// first statement when there are none.
    pub fn run(&mut self, source: String, base_dir: PathBuf, file: Option<PathBuf>, debug: bool) {
        self.start(Command::Run {
            source,
            base_dir,
            file,
            debug,
        });
    }

    pub fn python(&mut self, file: PathBuf, base_dir: PathBuf, interpreter: Option<String>) {
        self.start(Command::Python {
            file,
            base_dir,
            interpreter,
        });
    }

    pub fn set_breakpoints(&self, lines: &BTreeSet<usize>) {
        if let Ok(mut breakpoints) = self.breakpoints.lock() {
            breakpoints.clone_from(lines);
        }
    }

    /// Lets a paused program go on.
    pub fn resume(&self, how: Resume) {
        let _ = self.resume.send(how);
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
        if let Ok(mut child) = self.child.lock() {
            if let Some(child) = child.as_mut() {
                let _ = child.kill();
            }
        }
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
    resume: Arc<Mutex<Receiver<Resume>>>,
    interrupt: Arc<AtomicBool>,
    breakpoints: Arc<Mutex<BTreeSet<usize>>>,
    child: Arc<Mutex<Option<Child>>>,
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
                    debug,
                } => self.run(&source, base_dir, file, debug),
                Command::Python {
                    file,
                    base_dir,
                    interpreter,
                } => self.run_python(file, base_dir, interpreter),
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

    fn run(&self, source: &str, base_dir: PathBuf, file: Option<PathBuf>, debug: bool) {
        let lang = earth::detect_language(source);
        let mut output = self.output();
        let mut input = self.input();
        // Resumes sent before this run must not move it.
        if let Ok(resume) = self.resume.lock() {
            while resume.try_recv().is_ok() {}
        }

        let mut debugger = StudioDebugger {
            mode: Mode::Continue,
            breakpoints: Arc::clone(&self.breakpoints),
            resume: Arc::clone(&self.resume),
            interrupt: Arc::clone(&self.interrupt),
            events: self.events.clone(),
            wake: Arc::clone(&self.wake),
        };
        if self.breakpoints.lock().is_ok_and(|lines| lines.is_empty()) {
            debugger.mode = Mode::StepInto;
        }
        let result = earth::parse(source).and_then(|program| {
            let mut interpreter = Interpreter::new(&mut output, &mut input, lang);
            interpreter.set_base_dir(base_dir);
            interpreter.set_interrupt(Arc::clone(&self.interrupt));
            if debug {
                interpreter.set_debugger(&mut debugger);
            }
            interpreter.run(&program)
        });
        if let Err(error) = result {
            if error.message != earth::messages::Message::Interrupted {
                self.report(&error, lang, file);
            }
        }
    }

    fn run_python(&self, file: PathBuf, base_dir: PathBuf, interpreter: Option<String>) {
        let candidates = interpreter.map_or_else(
            || {
                if cfg!(windows) {
                    vec!["py".to_owned(), "python".to_owned()]
                } else {
                    vec!["python3".to_owned(), "python".to_owned()]
                }
            },
            |path| vec![path],
        );
        let mut process = None;
        let mut attempted = Vec::new();
        for candidate in candidates {
            attempted.push(candidate.clone());
            if let Ok(child) = ProcessCommand::new(&candidate)
                .arg(&file)
                .current_dir(&base_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn()
            {
                process = Some(child);
                break;
            }
        }
        let Some(mut process) = process else {
            self.send(Event::Error {
                    text: format!(
                        "Python yorumlayıcısı bulunamadı. Ayarlar'dan özel bir yol seçin veya https://www.python.org/downloads/ adresinden Python yükleyin. Denenenler: {}",
                        attempted.join(", ")
                    ),
                    line: None,
                    column: None,
                    file: Some(file),
                });
            return;
        };
        let stdout = process.stdout.take();
        let stderr = process.stderr.take();
        if let Ok(mut slot) = self.child.lock() {
            *slot = Some(process);
            if self.interrupt.load(Ordering::Relaxed) {
                if let Some(child) = slot.as_mut() {
                    let _ = child.kill();
                }
            }
        }
        let events = self.events.clone();
        let wake = Arc::clone(&self.wake);
        let out_thread = thread::spawn(move || pump_python_output(stdout, events, wake, false));
        let events = self.events.clone();
        let wake = Arc::clone(&self.wake);
        let err_thread = thread::spawn(move || pump_python_output(stderr, events, wake, true));
        loop {
            let done = self
                .child
                .lock()
                .ok()
                .and_then(|mut slot| {
                    slot.as_mut()
                        .map(|child| child.try_wait().ok().flatten().is_some())
                })
                .unwrap_or(true);
            if done {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let _ = out_thread.join();
        let _ = err_thread.join();
        if let Ok(mut slot) = self.child.lock() {
            if let Some(child) = slot.as_mut() {
                let _ = child.wait();
            }
            *slot = None;
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

#[derive(Clone, Copy, Debug, PartialEq)]
enum Mode {
    Continue,
    StepInto,
    /// Pause at a depth at most this.
    StepOver(usize),
    /// Pause at a depth below this.
    StepOut(usize),
}

struct StudioDebugger {
    mode: Mode,
    breakpoints: Arc<Mutex<BTreeSet<usize>>>,
    resume: Arc<Mutex<Receiver<Resume>>>,
    interrupt: Arc<AtomicBool>,
    events: Sender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
}

impl Debugger for StudioDebugger {
    fn should_pause(&mut self, line: usize, file: Option<&Path>, depth: usize) -> bool {
        if self.interrupt.load(Ordering::Relaxed) {
            return false;
        }
        // Breakpoints are lines of the main program, not of modules.
        let breakpoint = file.is_none()
            && self
                .breakpoints
                .lock()
                .is_ok_and(|lines| lines.contains(&line));
        breakpoint
            || match self.mode {
                Mode::Continue => false,
                Mode::StepInto => true,
                Mode::StepOver(limit) => depth <= limit,
                Mode::StepOut(limit) => depth < limit,
            }
    }

    fn pause(&mut self, snapshot: Snapshot) -> bool {
        let depth = snapshot.frames.len().saturating_sub(1);
        let _ = self.events.send(Event::Paused(snapshot));
        (self.wake)();
        let Ok(resume) = self.resume.lock() else {
            return false;
        };
        loop {
            if self.interrupt.load(Ordering::Relaxed) {
                return false;
            }
            match resume.recv_timeout(Duration::from_millis(50)) {
                Ok(how) => {
                    self.mode = match how {
                        Resume::Continue => Mode::Continue,
                        Resume::StepInto => Mode::StepInto,
                        Resume::StepOver => Mode::StepOver(depth),
                        Resume::StepOut => Mode::StepOut(depth),
                    };
                    return true;
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => return false,
            }
        }
    }
}

fn pump_python_output<R: Read + Send + 'static>(
    reader: Option<R>,
    events: Sender<Event>,
    wake: Arc<dyn Fn() + Send + Sync>,
    stderr: bool,
) {
    let Some(mut reader) = reader else { return };
    let mut buffer = [0; 4096];
    loop {
        match reader.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(count) => {
                let text = String::from_utf8_lossy(&buffer[..count]).into_owned();
                let event = if stderr {
                    Event::ErrorOutput(text)
                } else {
                    Event::Output(text)
                };
                let _ = events.send(event);
                wake();
            }
        }
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
                Event::Output(text) | Event::ErrorOutput(text) | Event::Value(text) => {
                    Some(text.as_str())
                }
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
            false,
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
        runner.run("iken doğru { }".to_owned(), ".".into(), None, false);
        std::thread::sleep(Duration::from_millis(50));
        runner.stop();
        let events = wait_until_finished(&mut runner);
        assert!(events
            .iter()
            .any(|event| matches!(event, Event::Finished { interrupted: true })));

        runner.run("girdi()".to_owned(), ".".into(), None, false);
        std::thread::sleep(Duration::from_millis(50));
        runner.stop();
        wait_until_finished(&mut runner);
    }

    /// Waits for the program to pause and returns where.
    fn wait_for_pause(runner: &mut Runner, events: &mut Vec<Event>) -> earth::debug::Snapshot {
        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            assert!(Instant::now() < deadline, "the program did not pause");
            for event in runner.poll() {
                if let Event::Paused(snapshot) = event {
                    return snapshot;
                }
                events.push(event);
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    #[test]
    fn debugs_with_breakpoints_and_steps() {
        use super::Resume;
        let source =
            "değişken t = 0\nfonksiyon f(x) {\n    döndür x + 1\n}\nt = f(t)\nt = f(t)\nyaz(t)";
        let mut runner = Runner::new(|| {});
        let mut events = Vec::new();

        // Without breakpoints a debug run stops at the first statement.
        runner.set_breakpoints(&Default::default());
        runner.run(source.to_owned(), ".".into(), None, true);
        assert_eq!(wait_for_pause(&mut runner, &mut events).line, 1);
        runner.resume(Resume::StepOver);
        assert_eq!(wait_for_pause(&mut runner, &mut events).line, 2);
        runner.resume(Resume::StepOver);
        assert_eq!(wait_for_pause(&mut runner, &mut events).line, 5);
        runner.resume(Resume::StepInto);
        let inside = wait_for_pause(&mut runner, &mut events);
        assert_eq!((inside.line, inside.frames.len()), (3, 2));
        runner.resume(Resume::StepOut);
        assert_eq!(wait_for_pause(&mut runner, &mut events).line, 6);
        runner.resume(Resume::Continue);
        events.extend(wait_until_finished(&mut runner));
        assert_eq!(output(&events), "2\n");

        // With a breakpoint it runs up to it, every time it is reached.
        runner.set_breakpoints(&[3].into());
        runner.run(source.to_owned(), ".".into(), None, true);
        let first = wait_for_pause(&mut runner, &mut events);
        assert_eq!(first.line, 3);
        assert!(first
            .variables
            .iter()
            .any(|v| v.name == "x" && v.value == "0"));
        runner.resume(Resume::Continue);
        let second = wait_for_pause(&mut runner, &mut events);
        assert!(second
            .variables
            .iter()
            .any(|v| v.name == "x" && v.value == "1"));
        // Stop while paused.
        runner.stop();
        let finished = wait_until_finished(&mut runner);
        assert!(finished
            .iter()
            .any(|event| matches!(event, Event::Finished { interrupted: true })));
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

    #[test]
    fn reports_when_python_interpreter_is_missing() {
        let root = std::env::temp_dir().join(format!("earth-studio-python-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("main.py");
        std::fs::write(&file, "print('unreachable')").unwrap();

        let mut runner = Runner::new(|| {});
        runner.python(
            file.clone(),
            root.clone(),
            Some(root.join("missing-python").display().to_string()),
        );
        let events = wait_until_finished(&mut runner);
        let message = events.iter().find_map(|event| match event {
            Event::Error {
                text,
                file: Some(error_file),
                ..
            } => Some((text.clone(), error_file.clone())),
            _ => None,
        });
        assert!(message.is_some());
        let (text, error_file) = message.unwrap();
        assert!(text.contains("Python"));
        assert_eq!(error_file, file);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn stops_python_processes_and_clears_the_child_slot() {
        let root =
            std::env::temp_dir().join(format!("earth-studio-python-stop-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let (script, interpreter, contents) = if cfg!(windows) {
            (
                root.join("loop.bat"),
                "cmd".to_owned(),
                "@echo off\r\n:loop\r\ngoto loop\r\n".to_owned(),
            )
        } else {
            (
                root.join("loop.sh"),
                "sh".to_owned(),
                "#!/bin/sh\nwhile true; do :; done\n".to_owned(),
            )
        };
        std::fs::write(&script, contents).unwrap();

        let mut runner = Runner::new(|| {});
        runner.python(script, root.clone(), Some(interpreter));
        std::thread::sleep(Duration::from_millis(50));
        runner.stop();
        let events = wait_until_finished(&mut runner);

        assert!(events
            .iter()
            .any(|event| matches!(event, Event::Finished { interrupted: true })));
        assert!(runner.child.lock().unwrap().is_none());
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn keeps_python_stdout_and_stderr_as_distinct_events() {
        use std::io::Cursor;
        use std::sync::mpsc;
        use std::sync::Arc;

        let (events, receiver) = mpsc::channel();
        let wake: Arc<dyn Fn() + Send + Sync> = Arc::new(|| {});
        super::pump_python_output(
            Some(Cursor::new(b"normal\n".to_vec())),
            events.clone(),
            Arc::clone(&wake),
            false,
        );
        super::pump_python_output(Some(Cursor::new(b"warning\n".to_vec())), events, wake, true);

        assert!(matches!(
            receiver.recv().unwrap(),
            Event::Output(text) if text == "normal\n"
        ));
        assert!(matches!(
            receiver.recv().unwrap(),
            Event::ErrorOutput(text) if text == "warning\n"
        ));
    }
}
