//! Automatic updates and the download counter.
//!
//! A background thread looks at the project's GitHub releases when Studio
//! starts and every few hours after that. When a newer version exists it
//! downloads the installer, checks it against the release's SHA256SUMS.txt
//! and reports it ready; Studio then installs it on request (restarting
//! itself) or, at the latest, when it closes. An installer that fails the
//! checksum is deleted and never run.

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use sha2::{Digest, Sha256};

pub const PROJECT_URL: &str = "https://github.com/MahmutSibal/earth-lang";
const RELEASES_API: &str =
    "https://api.github.com/repos/MahmutSibal/earth-lang/releases?per_page=100";
const INSTALLER: &str = "EarthStudio-Kurulum.exe";
const CHECKSUMS: &str = "SHA256SUMS.txt";
const CHECK_EVERY: Duration = Duration::from_secs(6 * 60 * 60);

/// Points update checks at another server, for tests and mirrors. The
/// server must answer like GitHub: `/releases/latest` redirects to
/// `/releases/tag/<version>`, files are under `/releases/download/<version>/`
/// and `/api/releases` lists releases like the GitHub API.
const URL_VARIABLE: &str = "EARTH_STUDIO_UPDATE_URL";
/// Set to `1` to turn automatic updates off, for example on managed machines.
const DISABLE_VARIABLE: &str = "EARTH_STUDIO_NO_UPDATE";

#[derive(Clone, Debug, PartialEq)]
pub enum Status {
    /// Not checked yet, or checking.
    Checking,
    UpToDate,
    /// A newer version exists but cannot be installed automatically here
    /// (outside Windows): the user is pointed to the releases page.
    Available {
        version: String,
    },
    Downloading {
        version: String,
        percent: u8,
    },
    /// Downloaded and verified; installs on request or when Studio closes.
    Ready {
        version: String,
    },
    /// No connection, or the download failed verification. Retried later.
    Failed,
    Disabled,
}

struct Shared {
    status: Status,
    installer: Option<PathBuf>,
    /// Total downloads of the installer across all releases.
    downloads: Option<u64>,
}

pub struct Updater {
    shared: Arc<Mutex<Shared>>,
    check_now: mpsc::Sender<()>,
}

impl Updater {
    /// Starts checking in the background. `wake` repaints the window when
    /// something changes. With `enabled` false (a setting) only the
    /// download count is read.
    pub fn start(enabled: bool, wake: impl Fn() + Send + Sync + 'static) -> Self {
        let disabled = !enabled || std::env::var(DISABLE_VARIABLE).is_ok_and(|value| value == "1");
        let shared = Arc::new(Mutex::new(Shared {
            status: if disabled {
                Status::Disabled
            } else {
                Status::Checking
            },
            installer: None,
            downloads: None,
        }));
        let (check_now, requests) = mpsc::channel::<()>();

        let source = Source::from_environment();
        let worker_state = Arc::clone(&shared);
        let _ = thread::Builder::new()
            .name("earth-updater".to_owned())
            .spawn(move || loop {
                if let Some(downloads) = source.downloads() {
                    if let Ok(mut state) = worker_state.lock() {
                        state.downloads = Some(downloads);
                    }
                    wake();
                }
                if !disabled {
                    check(
                        &source,
                        cfg!(windows),
                        &update_folder(),
                        &worker_state,
                        &wake,
                    );
                }
                match requests.recv_timeout(CHECK_EVERY) {
                    Ok(()) | Err(RecvTimeoutError::Timeout) => {}
                    Err(RecvTimeoutError::Disconnected) => break,
                }
            });
        Self { shared, check_now }
    }

    pub fn status(&self) -> Status {
        self.shared
            .lock()
            .map(|state| state.status.clone())
            .unwrap_or(Status::Failed)
    }

    pub fn downloads(&self) -> Option<u64> {
        self.shared.lock().ok().and_then(|state| state.downloads)
    }

    pub fn check_now(&self) {
        let _ = self.check_now.send(());
    }

    /// Starts the downloaded installer, if there is one. With `relaunch`
    /// the installer shows its progress and reopens Studio when done;
    /// without it, it installs silently (used when Studio is closing).
    /// Studio must exit right after this returns `true`.
    pub fn install(&self, relaunch: bool) -> bool {
        let Some(installer) = self
            .shared
            .lock()
            .ok()
            .and_then(|state| state.installer.clone())
        else {
            return false;
        };
        let arguments: &[&str] = if relaunch {
            &["/SILENT", "/SUPPRESSMSGBOXES", "/NORESTART", "/UPDATE"]
        } else {
            &["/VERYSILENT", "/SUPPRESSMSGBOXES", "/NORESTART"]
        };
        cfg!(windows)
            && std::process::Command::new(installer)
                .args(arguments)
                .spawn()
                .is_ok()
    }
}

/// Where releases come from: GitHub, or a test server.
struct Source {
    base: String,
    api: String,
}

impl Source {
    fn from_environment() -> Self {
        match std::env::var(URL_VARIABLE) {
            Ok(base) => Self::at(&base),
            Err(_) => Self {
                base: PROJECT_URL.to_owned(),
                api: RELEASES_API.to_owned(),
            },
        }
    }

    fn at(base: &str) -> Self {
        let base = base.trim_end_matches('/').to_owned();
        Self {
            api: format!("{base}/api/releases"),
            base,
        }
    }

    /// The newest release's tag, read from where `/releases/latest`
    /// redirects. Unlike the GitHub API this has no rate limit.
    fn latest_tag(&self) -> Option<String> {
        let response = agent(0)
            .get(&format!("{}/releases/latest", self.base))
            .call()
            .ok()?;
        let location = response.headers().get("location")?.to_str().ok()?;
        let (_, tag) = location.rsplit_once("/tag/")?;
        Some(tag.trim_end_matches('/').to_owned())
    }

    fn download_url(&self, tag: &str, file: &str) -> String {
        format!("{}/releases/download/{tag}/{file}", self.base)
    }

    /// Total downloads of the installer over all releases.
    fn downloads(&self) -> Option<u64> {
        let mut response = agent(5).get(&self.api).call().ok()?;
        if !response.status().is_success() {
            return None;
        }
        let releases: serde_json::Value =
            serde_json::from_reader(response.body_mut().as_reader()).ok()?;
        let total = releases
            .as_array()?
            .iter()
            .filter_map(|release| release["assets"].as_array())
            .flatten()
            .filter(|asset| asset["name"] == INSTALLER)
            .filter_map(|asset| asset["download_count"].as_u64())
            .sum();
        Some(total)
    }
}

pub fn agent(max_redirects: u32) -> ureq::Agent {
    ureq::Agent::config_builder()
        .max_redirects(max_redirects)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(15)))
        .tls_config(
            ureq::tls::TlsConfig::builder()
                .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                .build(),
        )
        .build()
        .into()
}

fn update_folder() -> PathBuf {
    std::env::temp_dir().join("earth-studio-update")
}

/// One round: find the newest version and, if it is newer than this one,
/// get its installer ready (or report it, where it cannot be installed).
fn check(
    source: &Source,
    can_install: bool,
    folder: &Path,
    shared: &Mutex<Shared>,
    wake: &dyn Fn(),
) {
    let set = |status: Status, installer: Option<PathBuf>| {
        if let Ok(mut state) = shared.lock() {
            state.status = status;
            state.installer = installer;
        }
        wake();
    };
    // An installer already downloaded in this session stays ready.
    if shared
        .lock()
        .is_ok_and(|state| matches!(state.status, Status::Ready { .. }))
    {
        return;
    }

    let Some(tag) = source.latest_tag() else {
        set(Status::Failed, None);
        return;
    };
    let version = tag.trim_start_matches('v').to_owned();
    if !is_newer(&version, env!("CARGO_PKG_VERSION")) {
        set(Status::UpToDate, None);
        return;
    }
    if !can_install {
        set(Status::Available { version }, None);
        return;
    }

    set(
        Status::Downloading {
            version: version.clone(),
            percent: 0,
        },
        None,
    );
    let progress = |percent: u8| {
        set(
            Status::Downloading {
                version: version.clone(),
                percent,
            },
            None,
        )
    };
    match download_verified(source, &tag, folder, &progress) {
        Ok(installer) => set(Status::Ready { version }, Some(installer)),
        Err(_) => set(Status::Failed, None),
    }
}

/// Downloads the installer of `tag` into `folder` and checks its SHA-256
/// against the release's checksums. A file that does not match is deleted.
fn download_verified(
    source: &Source,
    tag: &str,
    folder: &Path,
    progress: &dyn Fn(u8),
) -> io::Result<PathBuf> {
    let failed = |message: &str| io::Error::other(message.to_owned());
    fs::create_dir_all(folder)?;
    let target = folder.join(format!("EarthStudio-Kurulum-{tag}.exe"));

    let mut checksums = agent(5)
        .get(&source.download_url(tag, CHECKSUMS))
        .call()
        .map_err(|error| failed(&error.to_string()))?;
    if !checksums.status().is_success() {
        return Err(failed("no checksums"));
    }
    let checksums = checksums
        .body_mut()
        .read_to_string()
        .map_err(|error| failed(&error.to_string()))?;
    let expected = expected_hash(&checksums, INSTALLER).ok_or_else(|| failed("no checksum"))?;

    // Reuse an earlier, verified download of the same version.
    if fs::read(&target).is_ok_and(|bytes| sha256_hex(&bytes) == expected) {
        return Ok(target);
    }

    let mut response = agent(5)
        .get(&source.download_url(tag, INSTALLER))
        .call()
        .map_err(|error| failed(&error.to_string()))?;
    if !response.status().is_success() {
        return Err(failed("download failed"));
    }
    let total = response
        .headers()
        .get("content-length")
        .and_then(|value| value.to_str().ok()?.parse::<u64>().ok());

    let partial = target.with_extension("part");
    let mut file = fs::File::create(&partial)?;
    let mut hasher = Sha256::new();
    let mut reader = response.body_mut().as_reader();
    let mut buffer = vec![0; 64 * 1024];
    let mut received = 0u64;
    let mut last_percent = 0;
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        file.write_all(&buffer[..count])?;
        hasher.update(&buffer[..count]);
        received += count as u64;
        if let Some(total) = total.filter(|total| *total > 0) {
            let percent = (received * 100 / total).min(100) as u8;
            if percent != last_percent {
                last_percent = percent;
                progress(percent);
            }
        }
    }
    drop(file);

    if hex(&hasher.finalize()) != expected {
        let _ = fs::remove_file(&partial);
        return Err(failed("checksum mismatch"));
    }
    fs::rename(&partial, &target)?;
    Ok(target)
}

/// The hash listed for `file` in a `sha256sum`-style checksum file.
fn expected_hash(checksums: &str, file: &str) -> Option<String> {
    checksums.lines().find_map(|line| {
        let (hash, name) = line.split_once(char::is_whitespace)?;
        (name.trim().trim_start_matches('*') == file).then(|| hash.to_lowercase())
    })
}

fn sha256_hex(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// Whether `candidate` is a newer `major.minor.patch` than `current`.
fn is_newer(candidate: &str, current: &str) -> bool {
    fn parse(version: &str) -> Option<(u64, u64, u64)> {
        let mut parts = version
            .trim_start_matches('v')
            .split(['.', '-'])
            .map(|part| part.parse::<u64>().ok());
        Some((
            parts.next()??,
            parts.next()??,
            parts.next().flatten().unwrap_or(0),
        ))
    }
    match (parse(candidate), parse(current)) {
        (Some(candidate), Some(current)) => candidate > current,
        _ => false,
    }
}

/// Turns 1234567 into "1.234.567" (Turkish) or "1,234,567" (English).
pub fn group_digits(number: u64, separator: char) -> String {
    let digits = number.to_string();
    let mut grouped = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(separator);
        }
        grouped.push(digit);
    }
    grouped
}

#[cfg(test)]
mod tests {
    use super::{
        check, expected_hash, group_digits, is_newer, sha256_hex, Shared, Source, Status, INSTALLER,
    };
    use std::io::{BufRead, BufReader, Write};
    use std::net::TcpListener;
    use std::sync::Mutex;

    #[test]
    fn compares_versions() {
        assert!(is_newer("0.7.0", "0.6.0"));
        assert!(is_newer("v1.0", "0.9.9"));
        assert!(is_newer("0.6.10", "0.6.9"));
        assert!(!is_newer("0.6.0", "0.6.0"));
        assert!(!is_newer("0.5.9", "0.6.0"));
        assert!(!is_newer("garbage", "0.6.0"));
    }

    #[test]
    fn reads_checksum_files_and_groups_digits() {
        let sums = "abc  earth-studio-linux.tar.gz\nDEF *EarthStudio-Kurulum.exe\n";
        assert_eq!(expected_hash(sums, INSTALLER), Some("def".to_owned()));
        assert_eq!(expected_hash(sums, "missing"), None);
        assert_eq!(group_digits(1234567, '.'), "1.234.567");
        assert_eq!(group_digits(999, '.'), "999");
    }

    /// A tiny HTTP server that answers like GitHub releases.
    fn serve(installer: &'static [u8], checksum: String) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let location = format!("{base}/releases/tag/v9.9.9");
        std::thread::spawn(move || {
            for stream in listener.incoming().flatten() {
                let mut reader = BufReader::new(stream.try_clone().unwrap());
                let mut request = String::new();
                reader.read_line(&mut request).unwrap();
                // Skip the headers.
                let mut line = String::new();
                while reader.read_line(&mut line).is_ok_and(|n| n > 2) {
                    line.clear();
                }
                let path = request.split_whitespace().nth(1).unwrap_or("").to_owned();
                let (status, extra, body): (&str, String, Vec<u8>) = match path.as_str() {
                    "/releases/latest" => ("302 Found", format!("Location: {location}\r\n"), Vec::new()),
                    "/api/releases" => (
                        "200 OK",
                        String::new(),
                        br#"[{"assets":[{"name":"EarthStudio-Kurulum.exe","download_count":1200},{"name":"x","download_count":5}]},{"assets":[{"name":"EarthStudio-Kurulum.exe","download_count":34}]}]"#.to_vec(),
                    ),
                    "/releases/download/v9.9.9/SHA256SUMS.txt" => (
                        "200 OK",
                        String::new(),
                        format!("{checksum}  {INSTALLER}\n").into_bytes(),
                    ),
                    "/releases/download/v9.9.9/EarthStudio-Kurulum.exe" => {
                        ("200 OK", String::new(), installer.to_vec())
                    }
                    _ => ("404 Not Found", String::new(), Vec::new()),
                };
                let mut stream = stream;
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n{extra}\r\n",
                    body.len()
                );
                let _ = stream.write_all(&body);
            }
        });
        base
    }

    fn shared() -> Mutex<Shared> {
        Mutex::new(Shared {
            status: Status::Checking,
            installer: None,
            downloads: None,
        })
    }

    #[test]
    fn downloads_and_verifies_a_newer_installer() {
        let installer: &'static [u8] = b"pretend installer";
        let source = Source::at(&serve(installer, sha256_hex(installer)));
        let folder = std::env::temp_dir().join(format!("earth-update-ok-{}", std::process::id()));
        let state = shared();

        assert_eq!(source.downloads(), Some(1234));
        check(&source, true, &folder, &state, &|| {});
        let state = state.into_inner().unwrap();
        assert_eq!(
            state.status,
            Status::Ready {
                version: "9.9.9".to_owned()
            }
        );
        assert_eq!(std::fs::read(state.installer.unwrap()).unwrap(), installer);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn rejects_an_installer_that_fails_the_checksum() {
        let source = Source::at(&serve(b"tampered", sha256_hex(b"original")));
        let folder = std::env::temp_dir().join(format!("earth-update-bad-{}", std::process::id()));
        let state = shared();

        check(&source, true, &folder, &state, &|| {});
        let state = state.into_inner().unwrap();
        assert_eq!(state.status, Status::Failed);
        assert!(state.installer.is_none());
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(), 0);
        std::fs::remove_dir_all(folder).unwrap();
    }

    #[test]
    fn only_reports_versions_it_cannot_install() {
        let source = Source::at(&serve(b"x", sha256_hex(b"x")));
        let state = shared();
        check(&source, false, &std::env::temp_dir(), &state, &|| {});
        assert_eq!(
            state.into_inner().unwrap().status,
            Status::Available {
                version: "9.9.9".to_owned()
            }
        );
    }

    /// Talks to the real GitHub; run with `cargo test -- --ignored`.
    #[test]
    #[ignore]
    fn reads_the_real_latest_release() {
        let source = Source::from_environment();
        let tag = source.latest_tag().expect("latest tag");
        assert!(tag.starts_with('v'), "{tag}");
        assert!(source.downloads().is_some());
        eprintln!("latest: {tag}, downloads: {:?}", source.downloads());
    }
}
