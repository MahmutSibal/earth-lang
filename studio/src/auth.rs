//! Signing in with Google or GitHub.
//!
//! Google uses the flow for desktop apps: Studio listens on a loopback port,
//! the browser shows Google's sign-in page and returns to that port with a
//! code, which Studio exchanges (with PKCE) for a token. GitHub uses the
//! device flow: Studio shows a short code that the user enters on
//! github.com/login/device. Either way Studio only reads the user's name,
//! account and picture to show them. The GitHub token is kept (see
//! `credentials`) only so that the assistant can use GitHub Models.

use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::Value;
use sha2::{Digest, Sha256};

const GOOGLE_CLIENT_ID: &str =
    "877503077452-3reb0ondqrj59dhjhf1e4t7or7sqvbgf.apps.googleusercontent.com";
/// Google requires the secret of a desktop client even though it cannot be
/// kept secret in a desktop app. It is supplied when building releases, so
/// that it stays out of the repository; builds without it hide Google
/// sign-in.
const GOOGLE_CLIENT_SECRET: Option<&str> = option_env!("EARTH_GOOGLE_CLIENT_SECRET");
const GOOGLE_AUTHORIZE: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN: &str = "https://oauth2.googleapis.com/token";
const GOOGLE_USER: &str = "https://openidconnect.googleapis.com/v1/userinfo";

/// The Earth Studio GitHub App. Device flow must be enabled in its settings.
const GITHUB_CLIENT_ID: &str = "Iv23litx1H9X4gaVBVSr";
const GITHUB: GitHubUrls = GitHubUrls {
    device_code: "https://github.com/login/device/code",
    token: "https://github.com/login/oauth/access_token",
    user: "https://api.github.com/user",
};

/// Where the GitHub device flow talks to; tests use a local server.
struct GitHubUrls<'a> {
    device_code: &'a str,
    token: &'a str,
    user: &'a str,
}

/// How long a sign-in may wait for the user in the browser.
const TIMEOUT: Duration = Duration::from_secs(10 * 60);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Provider {
    Google,
    GitHub,
}

/// What Studio shows about the signed-in user.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub provider: Provider,
    pub name: String,
    /// The e-mail address (Google) or `@login` (GitHub).
    pub account: String,
    pub avatar_url: String,
}

/// A GitHub user access token. GitHub App tokens expire after a few hours
/// and come with a refresh token.
#[derive(Clone, Debug, PartialEq)]
pub struct Token {
    pub access: String,
    pub refresh: Option<String>,
    /// Seconds since the Unix epoch, if the token expires.
    pub expires_at: Option<u64>,
}

impl Token {
    fn from_response(response: &Value) -> Option<Self> {
        let now = unix_now();
        Some(Self {
            access: response["access_token"].as_str()?.to_owned(),
            refresh: response["refresh_token"].as_str().map(str::to_owned),
            expires_at: response["expires_in"].as_u64().map(|seconds| now + seconds),
        })
    }

    /// Expired, or about to expire within a minute.
    pub fn is_expired(&self) -> bool {
        self.expires_at
            .is_some_and(|expires_at| unix_now() + 60 >= expires_at)
    }

    pub fn to_json(&self) -> String {
        serde_json::json!({
            "access": self.access,
            "refresh": self.refresh,
            "expires_at": self.expires_at,
        })
        .to_string()
    }

    pub fn from_json(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        Some(Self {
            access: value["access"].as_str()?.to_owned(),
            refresh: value["refresh"].as_str().map(str::to_owned),
            expires_at: value["expires_at"].as_u64(),
        })
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Exchanges a refresh token for a new token. Fails if the refresh token
/// has expired too; the user then connects again.
pub fn refresh_github(token: &Token) -> Result<Token, Failure> {
    let refresh = token.refresh.as_deref().ok_or(Failure::Network)?;
    let response = post_form(
        GITHUB.token,
        &[
            ("client_id", GITHUB_CLIENT_ID),
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh),
        ],
    )?;
    Token::from_response(&response).ok_or(Failure::Network)
}

#[derive(Clone, Debug, PartialEq)]
pub enum Failure {
    /// The user declined in the browser.
    Denied,
    TimedOut,
    /// No connection, or the provider answered unexpectedly.
    Network,
    /// Device flow is not enabled for the GitHub App.
    DeviceFlowDisabled,
}

#[derive(Clone, Debug, PartialEq)]
pub enum State {
    Starting,
    /// Waiting for the user to finish in the browser.
    InBrowser {
        url: String,
    },
    /// Waiting for the user to enter `user_code` at `url`.
    DeviceCode {
        user_code: String,
        url: String,
    },
    /// Signed in; GitHub also gives a token for the assistant.
    Done(Profile, Option<Token>),
    Failed(Failure),
}

/// A sign-in running in the background.
pub struct SignIn {
    state: Arc<Mutex<State>>,
    cancelled: Arc<AtomicBool>,
}

type Wake = Arc<dyn Fn() + Send + Sync>;

impl SignIn {
    pub fn google_available() -> bool {
        GOOGLE_CLIENT_SECRET.is_some_and(|secret| !secret.is_empty())
    }

    pub fn start(provider: Provider, wake: Wake) -> Self {
        let state = Arc::new(Mutex::new(State::Starting));
        let cancelled = Arc::new(AtomicBool::new(false));
        let sign_in = Self {
            state: Arc::clone(&state),
            cancelled: Arc::clone(&cancelled),
        };
        let _ = thread::Builder::new()
            .name("earth-sign-in".to_owned())
            .spawn(move || {
                let report = |next: State| {
                    if let Ok(mut current) = state.lock() {
                        *current = next;
                    }
                    wake();
                };
                let result = match provider {
                    Provider::Google => google(&report, &cancelled).map(|profile| (profile, None)),
                    Provider::GitHub => github(&GITHUB, &report, &cancelled)
                        .map(|(profile, token)| (profile, Some(token))),
                };
                if !cancelled.load(Ordering::Relaxed) {
                    report(match result {
                        Ok((profile, token)) => State::Done(profile, token),
                        Err(failure) => State::Failed(failure),
                    });
                }
            });
        sign_in
    }

    pub fn state(&self) -> State {
        self.state
            .lock()
            .map(|state| state.clone())
            .unwrap_or(State::Failed(Failure::Network))
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }
}

impl Drop for SignIn {
    fn drop(&mut self) {
        self.cancel();
    }
}

// ----- Google -----

fn google(report: &dyn Fn(State), cancelled: &AtomicBool) -> Result<Profile, Failure> {
    let secret = GOOGLE_CLIENT_SECRET.ok_or(Failure::Network)?;
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|_| Failure::Network)?;
    let port = listener.local_addr().map_err(|_| Failure::Network)?.port();
    let redirect = format!("http://127.0.0.1:{port}");
    let verifier = base64_url(&random_bytes(32));
    let challenge = base64_url(&Sha256::digest(verifier.as_bytes()));
    let expected_state = base64_url(&random_bytes(16));

    let url = format!(
        "{GOOGLE_AUTHORIZE}?{}",
        query(&[
            ("client_id", GOOGLE_CLIENT_ID),
            ("redirect_uri", &redirect),
            ("response_type", "code"),
            ("scope", "openid email profile"),
            ("code_challenge", &challenge),
            ("code_challenge_method", "S256"),
            ("state", &expected_state),
            ("prompt", "select_account"),
        ])
    );
    report(State::InBrowser { url });

    let parameters = wait_for_redirect(&listener, cancelled)?;
    let parameter = |name: &str| {
        parameters
            .iter()
            .find(|(key, _)| key == name)
            .map(|(_, value)| value.as_str())
    };
    if parameter("state") != Some(expected_state.as_str()) {
        return Err(Failure::Network);
    }
    if parameter("error").is_some() {
        return Err(Failure::Denied);
    }
    let code = parameter("code").ok_or(Failure::Network)?;

    let token = post_form(
        GOOGLE_TOKEN,
        &[
            ("client_id", GOOGLE_CLIENT_ID),
            ("client_secret", secret),
            ("code", code),
            ("code_verifier", &verifier),
            ("redirect_uri", &redirect),
            ("grant_type", "authorization_code"),
        ],
    )?;
    let access_token = token["access_token"].as_str().ok_or(Failure::Network)?;
    let user = get_json(GOOGLE_USER, access_token)?;
    let email = user["email"].as_str().unwrap_or_default().to_owned();
    Ok(Profile {
        provider: Provider::Google,
        name: user["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .map(str::to_owned)
            .unwrap_or_else(|| email.clone()),
        account: email,
        avatar_url: user["picture"].as_str().unwrap_or_default().to_owned(),
    })
}

/// Answers requests on the loopback port until one carries the result of
/// the sign-in, and returns its query parameters.
fn wait_for_redirect(
    listener: &TcpListener,
    cancelled: &AtomicBool,
) -> Result<Vec<(String, String)>, Failure> {
    listener
        .set_nonblocking(true)
        .map_err(|_| Failure::Network)?;
    let deadline = Instant::now() + TIMEOUT;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Failure::Denied);
        }
        if Instant::now() > deadline {
            return Err(Failure::TimedOut);
        }
        match listener.accept() {
            Ok((stream, _)) => {
                if let Some(parameters) = handle_request(stream) {
                    return Ok(parameters);
                }
            }
            Err(_) => thread::sleep(Duration::from_millis(100)),
        }
    }
}

fn handle_request(mut stream: TcpStream) -> Option<Vec<(String, String)>> {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let mut request = Vec::new();
    let mut buffer = [0u8; 2048];
    while !request.windows(4).any(|window| window == b"\r\n\r\n") && request.len() < 16 * 1024 {
        match stream.read(&mut buffer) {
            Ok(0) | Err(_) => break,
            Ok(read) => request.extend_from_slice(&buffer[..read]),
        }
    }
    let request = String::from_utf8_lossy(&request);
    let target = request.lines().next()?.split_whitespace().nth(1)?;
    let parameters = target
        .split_once('?')
        .map(|(_, query)| parse_query(query))
        .unwrap_or_default();
    let finished = parameters
        .iter()
        .any(|(key, _)| key == "code" || key == "error");
    let (status, page) = if finished {
        ("200 OK", FINISHED_PAGE)
    } else {
        ("404 Not Found", "")
    };
    let _ = write!(
        stream,
        "HTTP/1.1 {status}\r\nContent-Type: text/html; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{page}",
        page.len()
    );
    finished.then_some(parameters)
}

const FINISHED_PAGE: &str = r#"<!doctype html>
<html><head><meta charset="utf-8"><title>Earth Studio</title></head>
<body style="font-family: system-ui, sans-serif; background: #111; color: #eee; display: grid; place-items: center; height: 100vh; margin: 0">
<div style="text-align: center">
<h1>🌍 Earth Studio</h1>
<p>Giriş tamamlandı. Bu sekmeyi kapatıp Earth Studio'ya dönebilirsiniz.</p>
<p style="color: #999">Signed in. You can close this tab and return to Earth Studio.</p>
</div></body></html>"#;

// ----- GitHub -----

fn github(
    urls: &GitHubUrls,
    report: &dyn Fn(State),
    cancelled: &AtomicBool,
) -> Result<(Profile, Token), Failure> {
    let device = post_form(urls.device_code, &[("client_id", GITHUB_CLIENT_ID)])?;
    if device["error"] == "device_flow_disabled" {
        return Err(Failure::DeviceFlowDisabled);
    }
    let device_code = device["device_code"].as_str().ok_or(Failure::Network)?;
    let user_code = device["user_code"].as_str().ok_or(Failure::Network)?;
    let url = device["verification_uri"]
        .as_str()
        .unwrap_or("https://github.com/login/device");
    let mut interval = device["interval"].as_u64().unwrap_or(5).max(1);
    let expires = device["expires_in"].as_u64().unwrap_or(900);
    report(State::DeviceCode {
        user_code: user_code.to_owned(),
        url: url.to_owned(),
    });

    let deadline = Instant::now() + Duration::from_secs(expires).min(TIMEOUT);
    let token = loop {
        sleep_unless_cancelled(Duration::from_secs(interval), cancelled)?;
        if Instant::now() > deadline {
            return Err(Failure::TimedOut);
        }
        let token = post_form(
            urls.token,
            &[
                ("client_id", GITHUB_CLIENT_ID),
                ("device_code", device_code),
                ("grant_type", "urn:ietf:params:oauth:grant-type:device_code"),
            ],
        )?;
        if let Some(token) = Token::from_response(&token) {
            break token;
        }
        match token["error"].as_str() {
            Some("authorization_pending") => {}
            Some("slow_down") => interval += 5,
            Some("access_denied") => return Err(Failure::Denied),
            Some("expired_token") => return Err(Failure::TimedOut),
            Some("device_flow_disabled") => return Err(Failure::DeviceFlowDisabled),
            _ => return Err(Failure::Network),
        }
    };

    let user = get_json(urls.user, &token.access)?;
    let login = user["login"].as_str().ok_or(Failure::Network)?;
    let avatar = user["avatar_url"].as_str().unwrap_or_default();
    let profile = Profile {
        provider: Provider::GitHub,
        name: user["name"]
            .as_str()
            .filter(|name| !name.is_empty())
            .unwrap_or(login)
            .to_owned(),
        account: format!("@{login}"),
        avatar_url: if avatar.is_empty() {
            String::new()
        } else {
            format!(
                "{avatar}{}s=128",
                if avatar.contains('?') { '&' } else { '?' }
            )
        },
    };
    Ok((profile, token))
}

fn sleep_unless_cancelled(duration: Duration, cancelled: &AtomicBool) -> Result<(), Failure> {
    let until = Instant::now() + duration;
    while Instant::now() < until {
        if cancelled.load(Ordering::Relaxed) {
            return Err(Failure::Denied);
        }
        thread::sleep(Duration::from_millis(200));
    }
    Ok(())
}

// ----- avatars -----

/// Downloads and decodes a profile picture.
pub fn fetch_avatar(url: &str) -> Option<eframe::egui::ColorImage> {
    if url.is_empty() {
        return None;
    }
    let mut response = crate::update::agent(5)
        .get(url)
        .header("User-Agent", "Earth-Studio")
        .call()
        .ok()?;
    if !response.status().is_success() {
        return None;
    }
    let bytes = response
        .body_mut()
        .with_config()
        .limit(5 * 1024 * 1024)
        .read_to_vec()
        .ok()?;
    let image = image::load_from_memory(&bytes).ok()?.to_rgba8();
    Some(eframe::egui::ColorImage::from_rgba_unmultiplied(
        [image.width() as usize, image.height() as usize],
        image.as_raw(),
    ))
}

// ----- HTTP and encoding helpers -----

fn post_form(url: &str, form: &[(&str, &str)]) -> Result<Value, Failure> {
    let mut response = crate::update::agent(5)
        .post(url)
        .header("Accept", "application/json")
        .header("User-Agent", "Earth-Studio")
        .send_form(form.iter().copied())
        .map_err(|_| Failure::Network)?;
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|_| Failure::Network)?;
    serde_json::from_str(&text).map_err(|_| Failure::Network)
}

fn get_json(url: &str, token: &str) -> Result<Value, Failure> {
    let mut response = crate::update::agent(5)
        .get(url)
        .header("Authorization", &format!("Bearer {token}"))
        .header("Accept", "application/json")
        .header("User-Agent", "Earth-Studio")
        .call()
        .map_err(|_| Failure::Network)?;
    if !response.status().is_success() {
        return Err(Failure::Network);
    }
    let text = response
        .body_mut()
        .read_to_string()
        .map_err(|_| Failure::Network)?;
    serde_json::from_str(&text).map_err(|_| Failure::Network)
}

fn random_bytes(count: usize) -> Vec<u8> {
    let mut bytes = vec![0u8; count];
    getrandom::fill(&mut bytes).expect("the system has no random source");
    bytes
}

/// Base64url without padding, as PKCE and OAuth `state` values use.
fn base64_url(bytes: &[u8]) -> String {
    const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let value = chunk.iter().enumerate().fold(0u32, |value, (index, byte)| {
            value | u32::from(*byte) << (16 - 8 * index)
        });
        for index in 0..=chunk.len() {
            text.push(ALPHABET[(value >> (18 - 6 * index) & 63) as usize] as char);
        }
    }
    text
}

fn query(parameters: &[(&str, &str)]) -> String {
    parameters
        .iter()
        .map(|(key, value)| format!("{}={}", percent_encode(key), percent_encode(value)))
        .collect::<Vec<_>>()
        .join("&")
}

fn percent_encode(text: &str) -> String {
    let mut encoded = String::new();
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.~".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

fn parse_query(query: &str) -> Vec<(String, String)> {
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=').unwrap_or((pair, ""));
            Some((percent_decode(key)?, percent_decode(value)?))
        })
        .collect()
}

fn percent_decode(text: &str) -> Option<String> {
    let mut bytes = Vec::new();
    let mut input = text.bytes();
    while let Some(byte) = input.next() {
        match byte {
            b'%' => {
                let high = (input.next()? as char).to_digit(16)?;
                let low = (input.next()? as char).to_digit(16)?;
                bytes.push((high * 16 + low) as u8);
            }
            b'+' => bytes.push(b' '),
            _ => bytes.push(byte),
        }
    }
    String::from_utf8(bytes).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_base64_url_without_padding() {
        assert_eq!(base64_url(b""), "");
        assert_eq!(base64_url(b"f"), "Zg");
        assert_eq!(base64_url(b"fo"), "Zm8");
        assert_eq!(base64_url(b"foo"), "Zm9v");
        assert_eq!(base64_url(&[0xfb, 0xff]), "-_8");
        // The PKCE example from RFC 7636, appendix B.
        let verifier = "dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk";
        assert_eq!(
            base64_url(&Sha256::digest(verifier.as_bytes())),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn encodes_and_decodes_queries() {
        assert_eq!(
            query(&[("scope", "openid email"), ("a", "ç/=")]),
            "scope=openid%20email&a=%C3%A7%2F%3D"
        );
        assert_eq!(
            parse_query("code=4%2F0Ab&state=x+y&error"),
            vec![
                ("code".to_owned(), "4/0Ab".to_owned()),
                ("state".to_owned(), "x y".to_owned()),
                ("error".to_owned(), String::new()),
            ]
        );
    }

    /// A stand-in for GitHub: answers each request with the next body.
    fn fake_server(answers: Vec<&'static str>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let server = thread::spawn(move || {
            let mut requests = Vec::new();
            for answer in answers {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = Vec::new();
                let mut buffer = [0u8; 4096];
                loop {
                    let read = stream.read(&mut buffer).unwrap();
                    request.extend_from_slice(&buffer[..read]);
                    let text = String::from_utf8_lossy(&request);
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
                write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{answer}",
                    answer.len()
                )
                .unwrap();
            }
            requests
        });
        (base, server)
    }

    #[test]
    fn github_device_flow_waits_for_the_user_and_reads_the_profile() {
        let (base, server) = fake_server(vec![
            r#"{"device_code":"dc","user_code":"ABCD-1234","verification_uri":"https://github.com/login/device","interval":1,"expires_in":60}"#,
            r#"{"error":"authorization_pending"}"#,
            r#"{"access_token":"tok","token_type":"bearer"}"#,
            r#"{"login":"MahmutSibal","name":null,"avatar_url":"https://avatars.example/u/1?v=4"}"#,
        ]);
        let urls = GitHubUrls {
            device_code: &format!("{base}/login/device/code"),
            token: &format!("{base}/login/oauth/access_token"),
            user: &format!("{base}/user"),
        };
        let states = Mutex::new(Vec::new());
        let report = |state: State| states.lock().unwrap().push(state);
        let (profile, token) = github(&urls, &report, &AtomicBool::new(false)).unwrap();
        assert_eq!(token.access, "tok");

        assert_eq!(
            profile,
            Profile {
                provider: Provider::GitHub,
                name: "MahmutSibal".to_owned(),
                account: "@MahmutSibal".to_owned(),
                avatar_url: "https://avatars.example/u/1?v=4&s=128".to_owned(),
            }
        );
        assert_eq!(
            states.into_inner().unwrap(),
            vec![State::DeviceCode {
                user_code: "ABCD-1234".to_owned(),
                url: "https://github.com/login/device".to_owned(),
            }]
        );
        let requests = server.join().unwrap();
        assert!(requests[0].contains("client_id=Iv23litx1H9X4gaVBVSr"));
        assert!(requests[2].contains("device_code=dc"));
        assert!(
            requests[3].contains("Authorization: Bearer tok")
                || requests[3].contains("authorization: Bearer tok")
        );
    }

    #[test]
    fn github_reports_a_disabled_device_flow() {
        let (base, server) = fake_server(vec![r#"{"error":"device_flow_disabled"}"#]);
        let urls = GitHubUrls {
            device_code: &format!("{base}/device"),
            token: &format!("{base}/token"),
            user: &format!("{base}/user"),
        };
        let result = github(&urls, &|_| {}, &AtomicBool::new(false));
        assert_eq!(result, Err(Failure::DeviceFlowDisabled));
        server.join().unwrap();
    }

    #[test]
    fn loopback_returns_the_code_and_ignores_other_requests() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let browser = thread::spawn(move || {
            for path in ["/favicon.ico", "/?code=abc&state=s1"] {
                let mut stream = TcpStream::connect(address).unwrap();
                write!(stream, "GET {path} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
                let mut answer = String::new();
                stream.read_to_string(&mut answer).unwrap();
                assert!(answer.starts_with("HTTP/1.1"));
            }
        });
        let parameters = wait_for_redirect(&listener, &AtomicBool::new(false)).unwrap();
        browser.join().unwrap();
        assert!(parameters.contains(&("code".to_owned(), "abc".to_owned())));
    }
}
