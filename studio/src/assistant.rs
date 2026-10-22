//! Earth Assistant: a chat about Earth programs, answered by a model from
//! an AI service the user picks (Google Gemini, OpenRouter, OpenAI, a local
//! Ollama or LM Studio, or any other OpenAI-compatible address) with the
//! user's own API key.
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

use crate::settings::Level;

/// The services the assistant can use. All of them speak the OpenAI chat
/// completions protocol, so only the address and the key differ. Model
/// names change often, so they are listed from the service, never fixed
/// here.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Service {
    /// Free with a key from Google AI Studio (Flash models).
    Gemini,
    /// Free models (`:free`) with an OpenRouter key.
    OpenRouter,
    OpenAI,
    /// Ollama or LM Studio on this computer: free, offline, no key.
    Local,
    /// Any OpenAI-compatible address.
    Custom,
}

impl Service {
    pub const ALL: [Service; 5] = [
        Service::Gemini,
        Service::OpenRouter,
        Service::OpenAI,
        Service::Local,
        Service::Custom,
    ];

    pub fn id(self) -> &'static str {
        match self {
            Service::Gemini => "gemini",
            Service::OpenRouter => "openrouter",
            Service::OpenAI => "openai",
            Service::Local => "local",
            Service::Custom => "custom",
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|service| service.id() == id)
    }

    /// The service's address; `Custom` has none of its own.
    pub fn base_url(self) -> Option<&'static str> {
        match self {
            Service::Gemini => Some("https://generativelanguage.googleapis.com/v1beta/openai/"),
            Service::OpenRouter => Some("https://openrouter.ai/api/v1/"),
            Service::OpenAI => Some("https://api.openai.com/v1/"),
            Service::Local => Some("http://localhost:11434/v1/"),
            Service::Custom => None,
        }
    }

    pub fn needs_key(self) -> bool {
        matches!(
            self,
            Service::Gemini | Service::OpenRouter | Service::OpenAI
        )
    }

    /// Where the user gets an API key.
    pub fn key_page(self) -> Option<&'static str> {
        match self {
            Service::Gemini => Some("https://aistudio.google.com/apikey"),
            Service::OpenRouter => Some("https://openrouter.ai/keys"),
            Service::OpenAI => Some("https://platform.openai.com/api-keys"),
            Service::Local | Service::Custom => None,
        }
    }
}

/// `base` + `path`, with exactly one slash between them.
pub fn url(base: &str, path: &str) -> String {
    format!("{}/{path}", base.trim().trim_end_matches('/'))
}

const REFERENCE: &str = include_str!("assistant_reference.md");
const MAX_FIXES: usize = 2;
/// Free tiers accept only a few thousand tokens per request, so the open
/// file and the history are cut to fit.
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
    /// The service refused the API key.
    InvalidKey,
    RateLimited,
    TooLong,
    UnknownModel,
    /// No connection: the reason, as the system reported it.
    Network(String),
    /// The service answered with an error not covered above: the HTTP
    /// status (none inside a stream) and its message, shown to the user.
    Failed(String),
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
}

/// One answer being written in the background.
pub struct Job {
    shared: Arc<Mutex<Shared>>,
    cancelled: Arc<AtomicBool>,
}

pub struct Request {
    pub base_url: String,
    pub api_key: Option<String>,
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
                let result = converse(&request, &cancelled, &update);
                update(&|state| state.result = Some(result.clone()));
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
    cancelled: &AtomicBool,
    update: &Update,
) -> Result<Entry, ChatError> {
    let mut messages = messages(&request.system, &request.history);
    let mut answer = complete(request, &messages, cancelled, update)?;
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
        answer = complete(request, &messages, cancelled, update)?;
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

/// Gets one answer: streamed as it is written, or, if what comes back is
/// not a readable stream (some proxies and security programs break
/// streams), asked again without streaming.
fn complete(
    request: &Request,
    messages: &[Value],
    cancelled: &AtomicBool,
    update: &Update,
) -> Result<String, ChatError> {
    match exchange(request, messages, true, cancelled, update)? {
        Outcome::Answer(answer) => Ok(answer),
        Outcome::NotAnAnswer(first) => {
            update(&|state| state.text.clear());
            match exchange(request, messages, false, cancelled, update)? {
                Outcome::Answer(answer) => Ok(answer),
                Outcome::NotAnAnswer(second) => Err(ChatError::Failed(format!(
                    "{first} | stream: off → {second}"
                ))),
            }
        }
    }
}

enum Outcome {
    Answer(String),
    /// A successful status, but no answer in it: what came instead.
    NotAnAnswer(String),
}

fn exchange(
    request: &Request,
    messages: &[Value],
    stream: bool,
    cancelled: &AtomicBool,
    update: &Update,
) -> Result<Outcome, ChatError> {
    let body = json!({
        "model": request.model,
        "messages": messages,
        "stream": stream,
    });
    // Redirects are not followed: a POST that is redirected loses its body,
    // and where it was sent is worth reporting.
    let response = authorized(
        crate::update::agent(0).post(&url(&request.base_url, "chat/completions")),
        request,
    )
    .header("Content-Type", "application/json")
    .send(body.to_string())
    .map_err(|error| ChatError::Network(error.to_string()))?;
    let status = response.status().as_u16();
    if response.status().is_redirection() {
        let location = header(&response, "location");
        return Err(ChatError::Failed(format!("HTTP {status} → {location}")));
    }
    if !response.status().is_success() {
        let mut response = response;
        let detail = response.body_mut().read_to_string().unwrap_or_default();
        return Err(classify(status, &detail));
    }

    let status = response.status().as_u16();
    let content_type = header(&response, "content-type");
    let host = {
        use ureq::ResponseExt;
        response.get_uri().host().unwrap_or("?").to_owned()
    };
    let mut answer = String::new();
    // Anything that is not an event, in case the answer is not a stream.
    let mut other = String::new();
    let reader = BufReader::new(response.into_body().into_reader());
    for line in reader.lines() {
        if cancelled.load(Ordering::Relaxed) {
            return Err(ChatError::Cancelled);
        }
        let line = line.map_err(|error| ChatError::Network(error.to_string()))?;
        let Some(data) = line.trim().strip_prefix("data:") else {
            other.push_str(&line);
            other.push('\n');
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
    if !answer.trim().is_empty() {
        return Ok(Outcome::Answer(answer));
    }
    // A whole JSON answer instead of a stream.
    if let Ok(whole) = serde_json::from_str::<Value>(&other) {
        if let Some(content) = whole["choices"][0]["message"]["content"].as_str() {
            update(&|state| state.text.push_str(content));
            return Ok(Outcome::Answer(content.to_owned()));
        }
        if whole.get("error").is_some() {
            return Err(classify(0, &other));
        }
    }
    let snippet: String = other.trim().chars().take(200).collect();
    Ok(Outcome::NotAnAnswer(format!(
        "HTTP {status} ({content_type}, {host}): {}",
        if snippet.is_empty() { "-" } else { &snippet }
    )))
}

/// Adds the key and the headers every service gets.
fn authorized<B>(builder: ureq::RequestBuilder<B>, request: &Request) -> ureq::RequestBuilder<B> {
    let mut builder = builder
        .header("Accept", "application/json, text/event-stream")
        .header("User-Agent", "Earth-Studio");
    if let Some(key) = request.api_key.as_deref().filter(|key| !key.is_empty()) {
        builder = builder.header("Authorization", &format!("Bearer {key}"));
    }
    // OpenRouter shows the app's name in its usage pages.
    if request.base_url.contains("openrouter.ai") {
        builder = builder
            .header("HTTP-Referer", crate::update::PROJECT_URL)
            .header("X-Title", "Earth Studio");
    }
    builder
}

/// The chat models a service offers, best guess for Earth first.
pub fn list_models(
    service: Service,
    base_url: &str,
    api_key: Option<&str>,
) -> Result<Vec<String>, ChatError> {
    let request = Request {
        base_url: base_url.to_owned(),
        api_key: api_key.map(str::to_owned),
        model: String::new(),
        system: String::new(),
        history: Vec::new(),
    };
    let mut response = authorized(
        crate::update::agent(0).get(&url(base_url, "models")),
        &request,
    )
    .call()
    .map_err(|error| ChatError::Network(error.to_string()))?;
    let status = response.status().as_u16();
    let body = response.body_mut().read_to_string().unwrap_or_default();
    if !response.status().is_success() {
        return Err(classify(status, &body));
    }
    let value: Value =
        serde_json::from_str(&body).map_err(|_| ChatError::Failed(describe(status, &body)))?;
    let mut models: Vec<String> = value["data"]
        .as_array()
        .or_else(|| value["models"].as_array())
        .into_iter()
        .flatten()
        .filter_map(|model| model["id"].as_str().or_else(|| model["name"].as_str()))
        .map(|id| id.strip_prefix("models/").unwrap_or(id).to_owned())
        .filter(|id| is_chat_model(id))
        .collect();
    models.sort();
    models.dedup();
    if let Some(best) = preferred_model(service, &models) {
        let best = best.to_owned();
        models.retain(|model| *model != best);
        models.insert(0, best);
    }
    Ok(models)
}

fn is_chat_model(id: &str) -> bool {
    let id = id.to_lowercase();
    ![
        "embed",
        "tts",
        "image",
        "audio",
        "whisper",
        "dall-e",
        "moderation",
        "transcribe",
        "imagen",
        "veo",
        "aqa",
        "realtime",
        "search",
    ]
    .iter()
    .any(|word| id.contains(word))
}

/// The model to start with: a fast, free one where the service has one.
fn preferred_model(service: Service, models: &[String]) -> Option<&str> {
    let find = |test: &dyn Fn(&str) -> bool| models.iter().map(String::as_str).find(|id| test(id));
    match service {
        Service::Gemini => find(&|id| {
            id.contains("flash")
                && !id.contains("lite")
                && !id.contains("preview")
                && !id.contains("exp")
        })
        .or_else(|| find(&|id| id.contains("flash"))),
        Service::OpenRouter => find(&|id| id.ends_with(":free")),
        Service::OpenAI => find(&|id| id.starts_with("gpt-") && id.contains("mini")),
        Service::Local | Service::Custom => None,
    }
    .or_else(|| models.first().map(String::as_str))
}

fn header(response: &ureq::http::Response<ureq::Body>, name: &str) -> String {
    response
        .headers()
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("-")
        .to_owned()
}

fn classify(status: u16, body: &str) -> ChatError {
    let detail = body.to_lowercase();
    let mentions = |words: &[&str]| words.iter().any(|word| detail.contains(word));
    if status == 413
        || mentions(&[
            "tokens_limit_reached",
            "context_length_exceeded",
            "too long",
        ])
    {
        ChatError::TooLong
    } else if status == 429 || mentions(&["rate limit", "ratelimit", "resource_exhausted", "quota"])
    {
        ChatError::RateLimited
    } else if status == 401
        || status == 403
        || mentions(&[
            "api key not valid",
            "invalid api key",
            "incorrect api key",
            "invalid_api_key",
        ])
    {
        ChatError::InvalidKey
    } else if mentions(&[
        "unknown_model",
        "model_not_found",
        "is not found for api version",
    ]) || (status == 404 && detail.contains("model"))
    {
        ChatError::UnknownModel
    } else {
        ChatError::Failed(describe(status, body))
    }
}

/// "HTTP 404: message" from an error answer, short enough to show.
fn describe(status: u16, body: &str) -> String {
    let message = serde_json::from_str::<Value>(body)
        .ok()
        .and_then(|value| {
            let error = &value["error"];
            [&error["message"], &error["code"], &value["message"], error]
                .into_iter()
                .find_map(|field| field.as_str().map(str::to_owned))
        })
        .unwrap_or_else(|| body.trim().to_owned());
    let message: String = message.chars().take(300).collect();
    match status {
        0 => message,
        status => format!("HTTP {status}: {message}"),
    }
}

/// The instructions and reference the model starts from; with `teacher`,
/// it teaches at that level instead of simply answering.
pub fn system_prompt(ui_lang: Lang, file: Option<OpenFile>, teacher: Option<Level>) -> String {
    let language = match ui_lang {
        Lang::Tr => "Turkish",
        Lang::En => "English",
    };
    let is_python = file
        .as_ref()
        .is_some_and(|file| file.name.to_ascii_lowercase().ends_with(".py"));
    let mut prompt = if is_python {
        format!(
            "You are a helpful Python programming assistant. Answer in {language}. \
             Give accurate, runnable Python examples and explain errors clearly."
        )
    } else {
        REFERENCE.replace("{ui_language}", language)
    };
    if !is_python {
        if let Some(level) = teacher {
            prompt.push_str(&teacher_instructions(level));
        }
    }
    if let Some(file) = file {
        let program_lang = if is_python {
            "Python"
        } else {
            match earth::detect_language(file.text) {
                Lang::Tr => "Turkish",
                Lang::En => "English",
            }
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
             use them in your code)\n```{}\n{text}{cut}\n",
            file.name,
            if is_python { "python" } else { "earth" }
        ));
    }
    prompt
}

/// How the teacher works, added to the reference in teacher mode.
fn teacher_instructions(level: Level) -> String {
    let student = match level {
        Level::Beginner => {
            "has never programmed before. Explain every new word in plain language, \
             use everyday comparisons, and keep each step very small"
        }
        Level::Intermediate => {
            "knows the basics (variables, conditions, loops). Build on that, \
             introduce one new idea at a time"
        }
        Level::Advanced => {
            "is comfortable programming. Be brief, focus on good structure, \
             edge cases and clear naming"
        }
    };
    format!(
        "
# Teacher mode
You are now a patient, encouraging programming teacher, not just a helper.
The student {student}.
- Teach in short steps: explain one idea, show a tiny example in a ```earth
  block, then give ONE small exercise and stop. Wait for the student.
- Do not give the full solution to an exercise unless the student asks for
  it twice; give hints and guiding questions instead.
- When the student shares their code and its output, first say what is
  good, then point to at most two problems, with hints to fix them. If it
  solves the exercise, congratulate them and offer the next step.
- Explain error messages simply: what Earth means and where to look.
- Use the student's language and Earth keywords in that language.
"
    )
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
            None,
        );
        assert!(prompt.contains("(Turkish by default)"));
        assert!(prompt.contains("hesap.earth"));
        assert!(prompt.contains("Turkish keywords"));
        assert!(prompt.contains("değişken x = 1"));
    }

    #[test]
    fn teacher_mode_adds_its_instructions() {
        let plain = system_prompt(Lang::Tr, None, None);
        assert!(!plain.contains("Teacher mode"));
        let teacher = system_prompt(Lang::Tr, None, Some(Level::Beginner));
        assert!(teacher.contains("Teacher mode"));
        assert!(teacher.contains("never programmed before"));
        assert!(system_prompt(Lang::Tr, None, Some(Level::Advanced)).contains("comfortable"));
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
        let endpoint = format!("http://{}/v1/", listener.local_addr().unwrap());
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
                let reply = if let Some(text) = answer.strip_prefix("TEXT ") {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{text}",
                        text.len()
                    )
                } else if let Some(location) = answer.strip_prefix("REDIRECT ") {
                    format!(
                        "HTTP/1.1 301 Moved\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                } else if let Some(json) = answer.strip_prefix("JSON ") {
                    format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{json}",
                        json.len()
                    )
                } else if let Some(error) = answer.strip_prefix("HTTP ") {
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

    fn request(base_url: String) -> Request {
        Request {
            base_url,
            api_key: Some("sk-test".to_owned()),
            model: "test-model".to_owned(),
            system: system_prompt(Lang::Tr, None, None),
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
        let phases = Mutex::new(Vec::new());
        let shared = Mutex::new(Shared {
            text: String::new(),
            phase: Phase::Writing,
            result: None,
        });
        let update = |change: &dyn Fn(&mut Shared)| {
            let mut state = shared.lock().unwrap();
            change(&mut state);
            phases.lock().unwrap().push(state.phase);
        };
        let entry = converse(&request, &AtomicBool::new(false), &update).unwrap();

        assert_eq!(entry.check, Check::Passed);
        assert!(entry.text.contains("yaz(i)"));
        assert_eq!(shared.lock().unwrap().text, entry.text);
        assert!(phases.lock().unwrap().contains(&Phase::Fixing));

        let requests = server.join().unwrap();
        assert!(requests[0]
            .to_lowercase()
            .contains("authorization: bearer sk-test"));
        assert!(
            requests[0].starts_with("POST /v1/chat/completions"),
            "{}",
            requests[0]
        );
        assert!(requests[0].contains("\"stream\":true"));
        assert!(requests[1].contains("parser rejected"), "{}", requests[1]);
    }

    #[test]
    fn gives_up_after_the_allowed_fixes() {
        let broken = "```earth\nx += 1\n```".to_owned();
        let (endpoint, server) = fake_models(vec![broken.clone(), broken.clone(), broken]);
        let request = request(endpoint);
        let entry = converse(&request, &AtomicBool::new(false), &|_| {}).unwrap();
        assert_eq!(entry.check, Check::Failed);
        assert_eq!(server.join().unwrap().len(), 1 + MAX_FIXES);
    }

    #[test]
    fn accepts_an_answer_that_is_not_streamed() {
        let answer = json!({ "choices": [{ "message": { "content": "Merhaba!" } }] });
        let (endpoint, server) = fake_models(vec![format!("JSON {answer}")]);
        let request = request(endpoint);
        let entry = converse(&request, &AtomicBool::new(false), &|_| {}).unwrap();
        assert_eq!(entry.text, "Merhaba!");
        server.join().unwrap();
    }

    #[test]
    fn asks_again_without_streaming_when_the_stream_is_unreadable() {
        let answer = json!({ "choices": [{ "message": { "content": "Merhaba!" } }] });
        let (endpoint, server) = fake_models(vec!["TEXT OK".to_owned(), format!("JSON {answer}")]);
        let request = request(endpoint);
        let entry = converse(&request, &AtomicBool::new(false), &|_| {}).unwrap();
        assert_eq!(entry.text, "Merhaba!");
        let requests = server.join().unwrap();
        assert!(requests[0].contains("\"stream\":true"));
        assert!(requests[1].contains("\"stream\":false"));
    }

    #[test]
    fn reports_what_came_back_instead_of_an_answer() {
        let (endpoint, server) = fake_models(vec!["TEXT OK".to_owned(), "TEXT OK".to_owned()]);
        let request = request(endpoint);
        let result = converse(&request, &AtomicBool::new(false), &|_| {});
        let Err(ChatError::Failed(detail)) = result else {
            panic!("{result:?}");
        };
        assert!(
            detail.contains("HTTP 200 (text/plain, 127.0.0.1): OK"),
            "{detail}"
        );
        assert!(detail.contains("stream: off"), "{detail}");
        server.join().unwrap();
    }

    #[test]
    fn reports_a_redirect_instead_of_following_it() {
        let (endpoint, server) =
            fake_models(vec!["REDIRECT https://example.com/elsewhere".to_owned()]);
        let request = request(endpoint);
        let result = converse(&request, &AtomicBool::new(false), &|_| {});
        assert_eq!(
            result,
            Err(ChatError::Failed(
                "HTTP 301 → https://example.com/elsewhere".to_owned()
            ))
        );
        server.join().unwrap();
    }

    #[test]
    fn reports_a_connection_failure_with_its_reason() {
        // Nothing listens on this port.
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}/chat", listener.local_addr().unwrap());
        drop(listener);
        let request = request(endpoint);
        let result = converse(&request, &AtomicBool::new(false), &|_| {});
        assert!(
            matches!(&result, Err(ChatError::Network(reason)) if !reason.is_empty()),
            "{result:?}"
        );
    }

    #[test]
    fn lists_chat_models_with_the_preferred_one_first() {
        let models = json!({ "data": [
            { "id": "models/gemini-2.5-pro" },
            { "id": "models/gemini-2.5-flash-lite" },
            { "id": "models/gemini-2.5-flash" },
            { "id": "models/text-embedding-004" },
            { "id": "models/imagen-4" },
        ]});
        let (base, server) = fake_models(vec![format!("JSON {models}")]);
        let listed = list_models(Service::Gemini, &base, Some("key")).unwrap();
        assert_eq!(
            listed,
            vec![
                "gemini-2.5-flash",
                "gemini-2.5-flash-lite",
                "gemini-2.5-pro"
            ]
        );
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /v1/models"), "{}", requests[0]);
        assert!(requests[0]
            .to_lowercase()
            .contains("authorization: bearer key"));
    }

    #[test]
    fn prefers_free_models_on_openrouter() {
        let models: Vec<String> = ["a/big", "b/small:free", "c/other"]
            .map(str::to_owned)
            .to_vec();
        assert_eq!(
            preferred_model(Service::OpenRouter, &models),
            Some("b/small:free")
        );
        assert_eq!(preferred_model(Service::Local, &models), Some("a/big"));
    }

    #[test]
    fn services_and_addresses() {
        assert!(!Service::Local.needs_key());
        assert!(Service::Gemini.needs_key());
        assert_eq!(url("http://x/v1/", "models"), "http://x/v1/models");
        assert_eq!(url("http://x/v1", "models"), "http://x/v1/models");
        assert_eq!(Service::from_id("openrouter"), Some(Service::OpenRouter));
    }

    #[test]
    fn explains_errors_from_ai_services() {
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
                ChatError::InvalidKey,
            ),
            (
                "HTTP 401 {\"message\":\"Bad credentials\"}",
                ChatError::InvalidKey,
            ),
            (
                "HTTP 400 {\"error\":{\"code\":\"unknown_model\"}}",
                ChatError::UnknownModel,
            ),
            (
                "HTTP 404 {\"error\":{\"code\":\"not_found\",\"message\":\"No such route\"}}",
                ChatError::Failed("HTTP 404: No such route".to_owned()),
            ),
            (
                "HTTP 500 oops",
                ChatError::Failed("HTTP 500: oops".to_owned()),
            ),
            (
                "HTTP 400 [{\"error\":{\"code\":400,\"message\":\"API key not valid. Please pass a valid API key.\"}}]",
                ChatError::InvalidKey,
            ),
            (
                "HTTP 400 {\"error\":{\"message\":\"User location is not supported for the API use.\"}}",
                ChatError::Failed("HTTP 400: User location is not supported for the API use.".to_owned()),
            ),
        ] {
            let (endpoint, server) = fake_models(vec![reply.to_owned()]);
            let request = request(endpoint);
                let result = converse(&request, &AtomicBool::new(false), &|_| {});
            assert_eq!(result, Err(expected), "{reply}");
            server.join().unwrap();
        }
    }
}
