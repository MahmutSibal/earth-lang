//! Earth Assistant: a chat about Earth programs, answered by a model from
//! GitHub Models with the user's GitHub sign-in.
//!
//! No model knows Earth, so every request starts with a short reference of
//! the language (`assistant_reference.md`). Before an answer is shown as
//! final, each Earth code block in it is run through Earth's own parser;
//! if one does not parse, the model is shown the errors and asked to
//! correct its answer, at most `MAX_FIXES` times.

use std::io::{BufRead, BufReader};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use earth::Lang;
use serde_json::{json, Value};

use crate::auth::{self, Token};

pub const ENDPOINT: &str = "https://models.github.ai/inference/chat/completions";

/// Models offered in the settings: (GitHub Models id, name shown).
pub const MODELS: &[(&str, &str)] = &[
    ("openai/gpt-4.1", "GPT-4.1"),
    ("openai/gpt-4.1-mini", "GPT-4.1 mini"),
    ("openai/gpt-4o", "GPT-4o"),
    ("openai/gpt-4o-mini", "GPT-4o mini"),
];
pub const DEFAULT_MODEL: &str = "openai/gpt-4.1";

const REFERENCE: &str = include_str!("assistant_reference.md");
const MAX_FIXES: usize = 2;
/// Free GitHub Models requests accept only a few thousand tokens, so the
/// open file and the history are cut to fit.
const FILE_LIMIT: usize = 8_000;
const HISTORY_LIMIT: usize = 8_000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Role {
    User,
    Assistant,
}

/// Whether the Earth code in an answer passed the parser.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Check {
    NoCode,
    Passed,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Entry {
    pub role: Role,
    pub text: String,
    pub check: Check,
}

impl Entry {
    pub fn user(text: String) -> Self {
        Self {
            role: Role::User,
            text,
            check: Check::NoCode,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Phase {
    Writing,
    /// The first answer had code that did not parse; asking for a fix.
    Fixing,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChatError {
    /// No token, or it expired and could not be refreshed.
    SignInNeeded,
    /// The token may not use GitHub Models (the Models permission).
    NoAccess,
    RateLimited,
    TooLong,
    UnknownModel,
    Network,
    Cancelled,
}

/// The open file, sent along so the model can talk about it.
pub struct OpenFile<'a> {
    pub name: &'a str,
    pub text: &'a str,
}

struct Shared {
    text: String,
    phase: Phase,
    result: Option<Result<Entry, ChatError>>,
    /// A token refreshed during the request, for Studio to keep.
    refreshed: Option<Token>,
}

/// One answer being written in the background.
pub struct Job {
    shared: Arc<Mutex<Shared>>,
    cancelled: Arc<AtomicBool>,
}

pub struct Request {
    pub endpoint: String,
    pub token: Token,
    pub model: String,
    pub system: String,
    pub history: Vec<Entry>,
}

impl Job {
    pub fn start(request: Request, wake: Arc<dyn Fn() + Send + Sync>) -> Self {
        let shared = Arc::new(Mutex::new(Shared {
            text: String::new(),
            phase: Phase::Writing,
            result: None,
            refreshed: None,
        }));
        let cancelled = Arc::new(AtomicBool::new(false));
        let job = Self {
            shared: Arc::clone(&shared),
            cancelled: Arc::clone(&cancelled),
        };
        let _ = thread::Builder::new()
            .name("earth-assistant".to_owned())
            .spawn(move || {
                let update = |change: &dyn Fn(&mut Shared)| {
                    if let Ok(mut state) = shared.lock() {
                        change(&mut state);
                    }
                    wake();
                };
                let mut token = request.token.clone();
                let result = converse(&request, &mut token, &cancelled, &update);
                update(&|state| {
                    if token != request.token {
                        state.refreshed = Some(token.clone());
                    }
                    state.result = Some(result.clone());
                });
            });
        job
    }

    /// The answer so far and what the assistant is doing.
    pub fn progress(&self) -> (String, Phase) {
        self.shared
            .lock()
            .map(|state| (state.text.clone(), state.phase))
            .unwrap_or((String::new(), Phase::Writing))
    }

    pub fn take_result(&self) -> Option<Result<Entry, ChatError>> {
        self.shared.lock().ok()?.result.take()
    }

    pub fn take_refreshed_token(&self) -> Option<Token> {
        self.shared.lock().ok()?.refreshed.take()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Drop for Job {
    fn drop(&mut self) {
        self.cancel();
    }
}

type Update<'a> = dyn Fn(&dyn Fn(&mut Shared)) + 'a;

fn converse(
    request: &Request,
    token: &mut Token,
    cancelled: &AtomicBool,
    update: &Update,
) -> Result<Entry, ChatError> {
    let mut messages = messages(&request.system, &request.history);
    let mut answer = complete(request, token, &messages, cancelled, update)?;
    for _ in 0..MAX_FIXES {
        let problems = check_code(&answer);
        if problems.is_empty() {
            break;
        }
        update(&|state| {
            state.phase = Phase::Fixing;
            state.text.clear();
        });
        messages.push(json!({ "role": "assistant", "content": answer }));
        messages.push(json!({ "role": "user", "content": fix_request(&problems) }));
        answer = complete(request, token, &messages, cancelled, update)?;
    }
    let check = if code_blocks(&answer).is_empty() {
        Check::NoCode
    } else if check_code(&answer).is_empty() {
        Check::Passed
    } else {
        Check::Failed
    };
    Ok(Entry {
        role: Role::Assistant,
        text: answer,
        check,
    })
}

fn fix_request(problems: &[String]) -> String {
    format!(
        "Earth's parser rejected the code in your answer:\n{}\n\
         Check the Earth reference again and reply with your whole answer \
         corrected. Do not mention this check.",
        problems.join("\n")
    )
}

fn messages(system: &str, history: &[Entry]) -> Vec<Value> {
    // The newest messages that fit, oldest first.
    let mut kept = Vec::new();
    let mut size = 0;
    for entry in history.iter().rev() {
        size += entry.text.len();
        if size > HISTORY_LIMIT && !kept.is_empty() {
            break;
        }
        kept.push(entry);
    }
    let mut messages = vec![json!({ "role": "system", "content": system })];
    messages.extend(kept.into_iter().rev().map(|entry| {
        json!({
            "role": match entry.role {
                Role::User => "user",
                Role::Assistant => "assistant",
            },
            "content": entry.text,
        })
    }));
    messages
}

/// Sends one request and streams the answer into the shared text.
fn complete(
    request: &Request,
    token: &mut Token,
    messages: &[Value],
    cancelled: &AtomicBool,
    update: &Update,
) -> Result<String, ChatError> {
    if token.is_expired() {
        *token = auth::refresh_github(token).map_err(|_| ChatError::SignInNeeded)?;
    }
    let body = json!({
        "model": request.model,
        "messages": messages,
        "stream": true,
    });
    let mut refreshed = false;
    let response = loop {
        let response = crate::update::agent(5)
            .post(&request.endpoint)
            .header("Authorization", &format!("Bearer {}", token.access))
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28")
            .header("Content-Type", "application/json")
            .header("User-Agent", "Earth-Studio")
            .send(body.to_string())
            .map_err(|_| ChatError::Network)?;
        let status = response.status().as_u16();
        if status == 401 && !refreshed && token.refresh.is_some() {
            *token = auth::refresh_github(token).map_err(|_| ChatError::SignInNeeded)?;
            refreshed = true;
            continue;
        }
        if response.status().is_success() {
            break response;
        }
        let mut response = response;
        let detail = response.body_mut().read_to_string().unwrap_or_default();
        return Err(classify(status, &detail));
    };

    let mut answer = String::new();
    let reader = BufReader::new(response.into_body().into_reader());
    for line in reader.lines() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(ChatError::Cancelled);
        }
        let line = line.map_err(|_| ChatError::Network)?;
        let Some(data) = line.trim().strip_prefix("data:") else {
            continue;
        };
        let data = data.trim();
        if data == "[DONE]" {
            break;
        }
        let Ok(event) = serde_json::from_str::<Value>(data) else {
            continue;
        };
        if event.get("error").is_some() {
            return Err(classify(0, data));
        }
        if let Some(delta) = event["choices"][0]["delta"]["content"].as_str() {
            answer.push_str(delta);
            update(&|state| state.text.push_str(delta));
        }
    }
    if answer.trim().is_empty() {
        return Err(ChatError::Network);
    }
    Ok(answer)
}

fn classify(status: u16, detail: &str) -> ChatError {
    let detail = detail.to_lowercase();
    if detail.contains("tokens_limit_reached") || status == 413 {
        ChatError::TooLong
    } else if detail.contains("unknown_model") || detail.contains("unavailable_model") {
        ChatError::UnknownModel
    } else if status == 429 || detail.contains("ratelimit") || detail.contains("rate limit") {
        ChatError::RateLimited
    } else if status == 401 {
        ChatError::SignInNeeded
    } else if status == 403 {
        ChatError::NoAccess
    } else {
        ChatError::Network
    }
}

/// The instructions and reference the model starts from.
pub fn system_prompt(ui_lang: Lang, file: Option<OpenFile>) -> String {
    let language = match ui_lang {
        Lang::Tr => "Turkish",
        Lang::En => "English",
    };
    let mut prompt = REFERENCE.replace("{ui_language}", language);
    if let Some(file) = file {
        let program_lang = match earth::detect_language(file.text) {
            Lang::Tr => "Turkish",
            Lang::En => "English",
        };
        let mut text = file.text;
        let mut cut = "";
        if text.len() > FILE_LIMIT {
            let mut end = FILE_LIMIT;
            while !text.is_char_boundary(end) {
                end -= 1;
            }
            text = &text[..end];
            cut = "\n# … (the rest of the file is not shown)";
        }
        prompt.push_str(&format!(
            "\n# The user's open file: {} (written with {program_lang} keywords; \
             use them in your code)\n```earth\n{text}{cut}\n```\n",
            file.name
        ));
    }
    prompt
}

/// A fenced code block in an answer.
#[derive(Clone, Debug, PartialEq)]
pub struct CodeBlock {
    pub code: String,
    /// Marked `earth`, or not marked at all.
    pub is_earth: bool,
}

/// The fenced code blocks in a text, in order. An unfinished block (while
/// the answer is still streaming) counts too.
pub fn code_blocks(text: &str) -> Vec<CodeBlock> {
    let mut blocks = Vec::new();
    let mut current: Option<(bool, Vec<&str>)> = None;
    for line in text.lines() {
        let trimmed = line.trim();
        match &mut current {
            None => {
                if let Some(tag) = trimmed.strip_prefix("```") {
                    let tag = tag.trim().to_lowercase();
                    current = Some((tag.is_empty() || tag == "earth", Vec::new()));
                }
            }
            Some((is_earth, lines)) => {
                if trimmed == "```" {
                    blocks.push(CodeBlock {
                        code: lines.join("\n"),
                        is_earth: *is_earth,
                    });
                    current = None;
                } else {
                    lines.push(line);
                }
            }
        }
    }
    if let Some((is_earth, lines)) = current {
        blocks.push(CodeBlock {
            code: lines.join("\n"),
            is_earth,
        });
    }
    blocks.retain(|block| !block.code.trim().is_empty());
    blocks.into_iter().filter(|block| block.is_earth).collect()
}

/// Parser errors in the Earth blocks of an answer, one line each.
pub fn check_code(text: &str) -> Vec<String> {
    code_blocks(text)
        .iter()
        .enumerate()
        .filter_map(|(index, block)| {
            earth::parse(&block.code)
                .err()
                .map(|error| format!("- code block {}: {}", index + 1, error.render(Lang::En)))
        })
        .collect()
}

/// A text split into prose and code, for drawing an answer.
#[derive(Debug, PartialEq)]
pub enum Part<'a> {
    Text(&'a str),
    Code(String),
}

pub fn parts(text: &str) -> Vec<Part<'_>> {
    let mut parts = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("```") {
        let before = &rest[..start];
        if !before.trim().is_empty() {
            parts.push(Part::Text(before.trim_end()));
        }
        let after_fence = &rest[start + 3..];
        let body_start = after_fence.find('\n').map_or(after_fence.len(), |i| i + 1);
        let body = &after_fence[body_start..];
        match body.find("```") {
            Some(end) => {
                parts.push(Part::Code(body[..end].trim_end().to_owned()));
                rest = &body[end + 3..];
                rest = rest.strip_prefix('\n').unwrap_or(rest);
            }
            None => {
                parts.push(Part::Code(body.trim_end().to_owned()));
                rest = "";
            }
        }
    }
    if !rest.trim().is_empty() {
        parts.push(Part::Text(rest.trim()));
    }
    parts
}

/// Lines of a whole-file change: removed, added or kept, for the preview
/// shown before an answer replaces the open file.
#[derive(Debug, PartialEq)]
pub enum DiffLine<'a> {
    Same(&'a str),
    Removed(&'a str),
    Added(&'a str),
}

pub fn diff<'a>(old: &'a str, new: &'a str) -> Vec<DiffLine<'a>> {
    let old: Vec<&str> = old.lines().collect();
    let new: Vec<&str> = new.lines().collect();
    // Longest common subsequence; files edited here are small.
    let mut table = vec![vec![0u32; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            table[i][j] = if old[i] == new[j] {
                table[i + 1][j + 1] + 1
            } else {
                table[i + 1][j].max(table[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut lines = Vec::new();
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            lines.push(DiffLine::Same(old[i]));
            i += 1;
            j += 1;
        } else if j < new.len() && (i == old.len() || table[i][j + 1] >= table[i + 1][j]) {
            lines.push(DiffLine::Added(new[j]));
            j += 1;
        } else {
            lines.push(DiffLine::Removed(old[i]));
            i += 1;
        }
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;

    #[test]
    fn reference_names_every_keyword_and_builtin() {
        for (spelling, builtin) in earth::keywords::builtins() {
            for lang in Lang::ALL {
                let name = builtin.name(lang);
                assert!(REFERENCE.contains(name), "{name} ({spelling}) is missing");
            }
        }
        for keyword in earth::keywords::all_keywords() {
            for lang in Lang::ALL {
                let name = keyword.name(lang);
                assert!(REFERENCE.contains(name), "{name} is missing");
            }
        }
    }

    #[test]
    fn finds_earth_code_blocks() {
        let text =
            "Şöyle:\n```earth\nyaz(1)\n```\nve\n```\nyaz(2)\n```\n```text\n1\n```\n```earth\nyaz(";
        let blocks: Vec<String> = code_blocks(text).into_iter().map(|b| b.code).collect();
        assert_eq!(blocks, vec!["yaz(1)", "yaz(2)", "yaz("]);
    }

    #[test]
    fn reports_code_that_does_not_parse() {
        assert!(check_code("```earth\nyaz(1)\n```").is_empty());
        let problems = check_code("```earth\nyaz(1)\nx += 1\n```");
        assert_eq!(problems.len(), 1);
        assert!(problems[0].contains("code block 1"), "{}", problems[0]);
        assert!(problems[0].contains("2:"), "{}", problems[0]);
    }

    #[test]
    fn splits_prose_and_code() {
        assert_eq!(
            parts("Merhaba\n```earth\nyaz(1)\n```\nSon"),
            vec![
                Part::Text("Merhaba"),
                Part::Code("yaz(1)".to_owned()),
                Part::Text("Son"),
            ]
        );
        assert_eq!(parts("```earth\nyaz("), vec![Part::Code("yaz(".to_owned())]);
    }

    #[test]
    fn diffs_line_by_line() {
        assert_eq!(
            diff("a\nb\nc", "a\nB\nc\nd"),
            vec![
                DiffLine::Same("a"),
                DiffLine::Added("B"),
                DiffLine::Removed("b"),
                DiffLine::Same("c"),
                DiffLine::Added("d"),
            ]
        );
    }

    #[test]
    fn prompt_includes_the_open_file_and_language() {
        let prompt = system_prompt(
            Lang::Tr,
            Some(OpenFile {
                name: "hesap.earth",
                text: "değişken x = 1",
            }),
        );
        assert!(prompt.contains("(Turkish by default)"));
        assert!(prompt.contains("hesap.earth"));
        assert!(prompt.contains("Turkish keywords"));
        assert!(prompt.contains("değişken x = 1"));
    }

    #[test]
    fn keeps_the_newest_history_that_fits() {
        let long = "x".repeat(HISTORY_LIMIT);
        let history = vec![Entry::user(long), Entry::user("son".to_owned())];
        let messages = messages("sys", &history);
        assert_eq!(messages.len(), 2);
        assert_eq!(messages[1]["content"], "son");
    }

    /// Serves each answer as a streamed chat completion, or a status line
    /// and body when the answer starts with a status code.
    fn fake_models(answers: Vec<String>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/chat/completions", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for answer in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 8192];
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    request.extend_from_slice(&buffer[..read]);
                    let text = String::from_utf8_lossy(&request).into_owned();
                    if let Some((head, body)) = text.split_once("\r\n\r\n") {
                        let length = head
                            .lines()
                            .find_map(|line| {
                                let (name, value) = line.split_once(':')?;
                                name.eq_ignore_ascii_case("content-length")
                                    .then(|| value.trim().parse::<usize>().ok())?
                            })
                            .unwrap_or(0);
                        if body.len() >= length {
                            break;
                        }
                    }
                }
                requests.push(String::from_utf8_lossy(&request).into_owned());
                let reply = if let Some(error) = answer.strip_prefix("HTTP ") {
                    let (status, body) = error.split_once(' ').unwrap();
                    format!(
                        "HTTP/1.1 {status} Error\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                } else {
                    let mut events = String::new();
                    // Send the answer in small pieces, as a model does.
                    let pieces: Vec<String> = answer
                        .chars()
                        .collect::<Vec<_>>()
                        .chunks(7)
                        .map(|chunk| chunk.iter().collect())
                        .collect();
                    for piece in pieces {
                        let event = json!({ "choices": [{ "delta": { "content": piece } }] });
                        events.push_str(&format!("data: {event}\n\n"));
                    }
                    events.push_str("data: [DONE]\n\n");
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{events}",
                        events.len()
                    )
                };
                stream.write_all(reply.as_bytes()).unwrap();
            }
            requests
        });
        (endpoint, server)
    }

    fn request(endpoint: String) -> Request {
        Request {
            endpoint,
            token: Token {
                access: "ghu_test".to_owned(),
                refresh: None,
                expires_at: None,
            },
            model: DEFAULT_MODEL.to_owned(),
            system: system_prompt(Lang::Tr, None),
            history: vec![Entry::user("1'den 3'e kadar yaz".to_owned())],
        }
    }

    #[test]
    fn asks_the_model_to_fix_code_that_does_not_parse() {
        let (endpoint, server) = fake_models(vec![
            "Şöyle:\n```earth\nher i içinde aralık(1, 4) {\n    i += 1\n}\n```".to_owned(),
            "Şöyle:\n```earth\nher i içinde aralık(1, 4) {\n    yaz(i)\n}\n```".to_owned(),
        ]);
        let request = request(endpoint);
        let mut token = request.token.clone();
        let phases = Mutex::new(Vec::new());
        let shared = Mutex::new(Shared {
            text: String::new(),
            phase: Phase::Writing,
            result: None,
            refreshed: None,
        });
        let update = |change: &dyn Fn(&mut Shared)| {
            let mut state = shared.lock().unwrap();
            change(&mut state);
            phases.lock().unwrap().push(state.phase);
        };
        let entry = converse(&request, &mut token, &AtomicBool::new(false), &update).unwrap();

        assert_eq!(entry.check, Check::Passed);
        assert!(entry.text.contains("yaz(i)"));
        assert_eq!(shared.lock().unwrap().text, entry.text);
        assert!(phases.lock().unwrap().contains(&Phase::Fixing));

        let requests = server.join().unwrap();
        assert!(requests[0]
            .to_lowercase()
            .contains("authorization: bearer ghu_test"));
        assert!(requests[0].contains("\"stream\":true"));
        assert!(requests[1].contains("parser rejected"), "{}", requests[1]);
    }

    #[test]
    fn gives_up_after_the_allowed_fixes() {
        let broken = "```earth\nx += 1\n```".to_owned();
        let (endpoint, server) = fake_models(vec![broken.clone(), broken.clone(), broken]);
        let request = request(endpoint);
        let mut token = request.token.clone();
        let entry = converse(&request, &mut token, &AtomicBool::new(false), &|_| {}).unwrap();
        assert_eq!(entry.check, Check::Failed);
        assert_eq!(server.join().unwrap().len(), 1 + MAX_FIXES);
    }

    #[test]
    fn explains_errors_from_github_models() {
        for (reply, expected) in [
            (
                "HTTP 429 {\"error\":{\"code\":\"RateLimitReached\"}}",
                ChatError::RateLimited,
            ),
            (
                "HTTP 413 {\"error\":{\"code\":\"tokens_limit_reached\"}}",
                ChatError::TooLong,
            ),
            (
                "HTTP 403 {\"error\":{\"code\":\"no_access\"}}",
                ChatError::NoAccess,
            ),
            (
                "HTTP 401 {\"message\":\"Bad credentials\"}",
                ChatError::SignInNeeded,
            ),
            (
                "HTTP 400 {\"error\":{\"code\":\"unknown_model\"}}",
                ChatError::UnknownModel,
            ),
        ] {
            let (endpoint, server) = fake_models(vec![reply.to_owned()]);
            let request = request(endpoint);
            let mut token = request.token.clone();
            let result = converse(&request, &mut token, &AtomicBool::new(false), &|_| {});
            assert_eq!(result, Err(expected), "{reply}");
            server.join().unwrap();
        }
    }
}
