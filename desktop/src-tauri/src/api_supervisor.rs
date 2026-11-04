//! Keeps the mobile-facing ADTP gateway (`adtp-api`) running.
//!
//! M.A.R.S talks to the desktop over the gateway's REST + `/relay` WebSocket
//! on port 40003. Previously the desktop shipped a "relay listener" that
//! connected *to* a gateway nobody started, so pairing succeeded on the phone
//! but ADTP never appeared online. This supervisor makes the gateway the
//! long-lived listener:
//!
//! 1. reuse a gateway that is already healthy and trusts our token;
//! 2. otherwise launch one from a bundled sidecar, `adtp-api` on PATH, the
//!    repo's venv, or `uv run --project packages/adtp-api`;
//! 3. detach it so it survives closing the desktop window;
//! 4. wait for `/api/v1/health` before reporting success.
//!
//! The token is shared with the QR generator through the settings store, so
//! the QR the phone scans always matches the gateway that is actually running.

use std::io::{Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter};

use crate::settings;

const HEALTH_PATH: &str = "/api/v1/health";
/// Authenticated endpoint used to detect a gateway running with a different
/// pairing token. Without this the desktop would happily "reuse" a listener
/// that rejects every request the UI makes.
const AUTH_PROBE_PATH: &str = "/api/v1/devices";
/// Per-candidate health wait. A frozen PyInstaller gateway is up in ~1-2 s,
/// so 25 s leaves plenty of headroom without freezing the QR flow for minutes
/// when every launcher is broken.
const START_TIMEOUT: Duration = Duration::from_secs(25);
/// Hard ceiling across all launchers: the UI must always get an answer.
const TOTAL_START_BUDGET: Duration = Duration::from_secs(90);
const PROBE_TIMEOUT: Duration = Duration::from_millis(1200);

#[derive(Clone, serde::Serialize)]
pub struct ApiStatus {
    pub running: bool,
    /// Local management URL (`http://127.0.0.1:40003`).
    pub url: String,
    /// URL a phone must use (`http://192.168.x.x:40003`).
    pub phone_url: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Probe {
    Healthy,
    Unauthorized,
    Offline,
}

struct Launcher {
    program: String,
    args: Vec<String>,
    envs: Vec<(String, String)>,
    label: String,
}

impl Launcher {
    fn new(program: impl Into<String>) -> Self {
        let program = program.into();
        Self {
            label: program.clone(),
            program,
            args: Vec::new(),
            envs: Vec::new(),
        }
    }

    fn args(mut self, args: Vec<String>) -> Self {
        self.args = args;
        self
    }

    fn env(mut self, key: &str, value: impl Into<String>) -> Self {
        self.envs.push((key.to_string(), value.into()));
        self
    }
}

fn parse_http_addr(url: &str) -> Option<(String, u16, bool)> {
    let trimmed = url.trim();
    let (scheme, rest) = trimmed.split_once("://")?;
    // Only the gateway's HTTP(S) URLs are valid here. Without this check a
    // `ws://…` URL would be treated as plain HTTP and quietly probed on the
    // wrong scheme.
    let secure = if scheme.eq_ignore_ascii_case("https") {
        true
    } else if scheme.eq_ignore_ascii_case("http") {
        false
    } else {
        return None;
    };
    let authority = rest.split('/').next()?;
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, port)) => (host, port.parse::<u16>().ok()?),
        None => (authority, if secure { 443 } else { 80 }),
    };
    let host = host.trim_start_matches('[').trim_end_matches(']');
    if host.is_empty() {
        return None;
    }
    Some((host.to_string(), port, secure))
}

fn tcp_open(url: &str) -> bool {
    let Some((host, port, _)) = parse_http_addr(url) else {
        return false;
    };
    let Ok(addrs) = (host.as_str(), port).to_socket_addrs() else {
        return false;
    };
    addrs
        .into_iter()
        .any(|addr| TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).is_ok())
}

/// Minimal HTTP status probe. Deliberately avoids pulling an HTTP client into
/// the desktop crate: the gateway runs on localhost and we only need a code.
fn http_status(url: &str, path: &str, token: Option<&str>) -> Option<u16> {
    let (host, port, secure) = parse_http_addr(url)?;
    if secure {
        // Local gateways are HTTP. For an operator-provided HTTPS URL a TCP
        // probe is enough to avoid pulling a TLS stack into the desktop.
        return if tcp_open(url) { Some(200) } else { None };
    }
    let addr = (host.as_str(), port).to_socket_addrs().ok()?.next()?;
    let mut stream = TcpStream::connect_timeout(&addr, PROBE_TIMEOUT).ok()?;
    stream.set_read_timeout(Some(PROBE_TIMEOUT)).ok()?;
    stream.set_write_timeout(Some(PROBE_TIMEOUT)).ok()?;

    let auth = token
        .map(|t| format!("Authorization: Bearer {t}\r\n"))
        .unwrap_or_default();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {host}\r\n{auth}Connection: close\r\n\r\n"
    )
    .ok()?;
    let mut buffer = [0u8; 64];
    let read = stream.read(&mut buffer).ok()?;
    let head = String::from_utf8_lossy(&buffer[..read]);
    head.split_whitespace().nth(1)?.parse().ok()
}

fn probe(url: &str, token: &str) -> Probe {
    match http_status(url, HEALTH_PATH, None) {
        Some(200) => match http_status(url, AUTH_PROBE_PATH, Some(token)) {
            Some(401 | 403) => Probe::Unauthorized,
            Some(200) => Probe::Healthy,
            _ => Probe::Offline,
        },
        _ => Probe::Offline,
    }
}

/// Read a token written by the desktop (`~/.config/adtp/api.env`) or by the
/// systemd installer. Keeping one canonical token means a listener started
/// before the desktop UI still accepts the QR the desktop shows.
fn env_file_token() -> Option<String> {
    let path = dirs::config_dir()?.join("adtp").join("api.env");
    let contents = std::fs::read_to_string(path).ok()?;
    for line in contents.lines() {
        let line = line.trim();
        if let Some(value) = line.strip_prefix("ADTP_API_TOKEN=") {
            let token = value.trim().trim_matches('"').trim_matches('\'');
            if token.len() >= 32 {
                return Some(token.to_string());
            }
        }
    }
    None
}

/// Returns a token shared by the QR payload, the gateway process and the
/// desktop UI. Generates one on first launch so pairing works out of the box.
fn ensure_token(app: &AppHandle) -> Result<String, String> {
    if let Some(token) = settings::load_api_token(app) {
        let trimmed = token.trim();
        if trimmed.len() >= 32 {
            return Ok(trimmed.to_string());
        }
    }
    if let Some(token) = env_file_token() {
        // Adopt the token a previously installed systemd listener is using.
        let _ = settings::save_api_token(app, &token);
        return Ok(token);
    }
    let token = settings::generate_token();
    settings::save_api_token(app, &token)?;
    // Keep the CLI usable with the same credential (`adtp status`, `adtp send`).
    let _ = settings::write_token_to_cli_config(&token);
    Ok(token)
}

fn find_repo_root() -> Option<PathBuf> {
    if let Ok(configured) = std::env::var("ADTP_REPO_DIR") {
        let path = PathBuf::from(configured);
        if path.join("packages/adtp-api").is_dir() {
            return Some(path);
        }
    }

    let mut starts = Vec::new();
    if let Ok(cwd) = std::env::current_dir() {
        starts.push(cwd);
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }

    for start in starts {
        let mut current: Option<&Path> = Some(start.as_path());
        for _ in 0..8 {
            let Some(dir) = current else { break };
            if dir.join("packages/adtp-api").is_dir() {
                return Some(dir.to_path_buf());
            }
            current = dir.parent();
        }
    }
    None
}

fn python_path(repo: &Path) -> String {
    let paths = [
        repo.join("packages/adtp-api/src"),
        repo.join("packages/adtp-mcp/src"),
    ];
    std::env::join_paths(paths)
        .map(|joined| joined.to_string_lossy().to_string())
        .unwrap_or_default()
}

/// A venv interpreter is only useful if its ABI still matches the installed
/// wheels. A system Python upgrade can leave `<repo>/.venv` pointing at a new
/// major version with no packages; probing first avoids a 20 s dead wait.
fn python_usable(python: &Path, repo: &Path) -> bool {
    Command::new(python)
        .args([
            "-c",
            "import adtp_api.main, fastapi, uvicorn, websockets, pydantic_settings",
        ])
        .env("PYTHONPATH", python_path(repo))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(Path::to_path_buf))
}

fn push_unique(out: &mut Vec<Launcher>, launcher: Launcher) {
    let duplicate = out
        .iter()
        .any(|existing| existing.program == launcher.program && existing.args == launcher.args);
    if !duplicate {
        out.push(launcher);
    }
}

/// Absolute locations the gateway may be installed at.
///
/// The release pipeline copies the frozen `adtp-api` next to `adtp-desktop`
/// (and into `/usr/bin` for the `.deb`), so a packaged install has no Python
/// dependency at all. The remaining paths keep `pipx`, the source installer
/// and older installs working.
fn gateway_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    if let Some(dir) = exe_dir() {
        for name in ["adtp-api", "adtp-api.exe"] {
            paths.push(dir.join(name));
        }
        for sub in ["resources", "binaries"] {
            for name in ["adtp-api", "adtp-api.exe"] {
                paths.push(dir.join(sub).join(name));
            }
        }
    }

    if let Some(data) = dirs::data_dir() {
        let adtp = data.join("adtp");
        paths.push(adtp.join("adtp-api"));
        paths.push(adtp.join("adtp-api.exe"));
        paths.push(adtp.join("venv").join("bin").join("adtp-api"));
        paths.push(adtp.join("venv").join("Scripts").join("adtp-api.exe"));
    }

    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".local").join("bin").join("adtp-api"));
        paths.push(
            home.join(".local")
                .join("share")
                .join("adtp")
                .join("venv")
                .join("bin")
                .join("adtp-api"),
        );
        paths.push(home.join(".cargo").join("bin").join("adtp-api"));
    }

    for fixed in [
        "/usr/local/bin/adtp-api",
        "/usr/bin/adtp-api",
        "/usr/lib/adtp/adtp-api",
        "/opt/adtp/bin/adtp-api",
    ] {
        paths.push(PathBuf::from(fixed));
    }

    paths
}

fn launchers(repo: Option<&Path>) -> Vec<Launcher> {
    let mut out = Vec::new();

    if let Ok(bin) = std::env::var("ADTP_API_BIN") {
        let trimmed = bin.trim();
        if !trimmed.is_empty() {
            push_unique(&mut out, Launcher::new(trimmed));
        }
    }

    // A sidecar bundled next to the desktop executable wins over anything
    // machine-specific: this is what the packaged installer ships.
    for path in gateway_paths() {
        if path.is_file() {
            push_unique(&mut out, Launcher::new(path.to_string_lossy().to_string()));
        }
    }

    // An installed console script (`pipx install`, `/usr/bin`, …).
    push_unique(&mut out, Launcher::new("adtp-api"));

    // Development checkout: the repo-local interpreter is the fastest path.
    if let Some(repo) = repo {
        for name in ["python3.13", "python3.12", "python3.11", "python"] {
            let python = repo.join(".venv").join("bin").join(name);
            if python.is_file() && python_usable(&python, repo) {
                push_unique(
                    &mut out,
                    Launcher::new(python.to_string_lossy().to_string())
                        .args(vec!["-m".into(), "adtp_api.main".into()])
                        .env("PYTHONPATH", python_path(repo)),
                );
                break;
            }
        }

        // A system Python with the gateway installed also works for dev
        // checkouts; if the dependencies are missing it exits immediately and
        // the supervisor moves on.
        push_unique(
            &mut out,
            Launcher::new("python3")
                .args(vec!["-m".into(), "adtp_api.main".into()])
                .env("PYTHONPATH", python_path(repo)),
        );

        // uv creates/repairs an isolated environment on demand, so a system
        // Python upgrade (or a half-created venv) can never leave the desktop
        // without a gateway. This is the fallback of last resort.
        let uv = resolve_uv();
        push_unique(
            &mut out,
            Launcher::new(uv).args(vec![
                "run".into(),
                "--project".into(),
                repo.join("packages/adtp-api").to_string_lossy().to_string(),
                "adtp-api".into(),
            ]),
        );
    }

    out
}

/// `uv` is commonly installed in `~/.local/bin`, which is not always on the
/// PATH of a desktop-launched Tauri process.
fn resolve_uv() -> String {
    if let Ok(home) = std::env::var("HOME") {
        let local = PathBuf::from(home).join(".local").join("bin").join("uv");
        if local.is_file() {
            return local.to_string_lossy().to_string();
        }
    }
    for candidate in ["/usr/local/bin/uv", "/usr/bin/uv"] {
        if Path::new(candidate).is_file() {
            return candidate.to_string();
        }
    }
    "uv".to_string()
}

fn write_env_file(token: &str, port: u16) -> Option<PathBuf> {
    let dir = dirs::config_dir()?.join("adtp");
    std::fs::create_dir_all(&dir).ok()?;
    let path = dir.join("api.env");
    let contents = format!("ADTP_API_HOST=0.0.0.0\nADTP_API_PORT={port}\nADTP_API_TOKEN={token}\n");
    std::fs::write(&path, contents).ok()?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600));
    }
    Some(path)
}

fn spawn_detached(
    launcher: &Launcher,
    token: &str,
    port: u16,
    log_path: &Path,
) -> std::io::Result<Child> {
    let log = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;
    let log_err = log.try_clone()?;

    let mut command = Command::new(&launcher.program);
    command
        .args(&launcher.args)
        .env("ADTP_API_HOST", "0.0.0.0")
        .env("ADTP_API_PORT", port.to_string())
        .env("ADTP_API_TOKEN", token)
        .env("ADTP_LOG_LEVEL", "INFO")
        .stdin(Stdio::null())
        .stdout(Stdio::from(log))
        .stderr(Stdio::from(log_err));

    for (key, value) in &launcher.envs {
        command.env(key, value);
    }

    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        // New session: closing the terminal or the desktop window must not
        // take the phone-facing listener with it.
        unsafe {
            command.pre_exec(|| {
                if libc::setsid() == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const DETACHED_PROCESS: u32 = 0x0000_0008;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(DETACHED_PROCESS | CREATE_NO_WINDOW);
    }

    command.spawn()
}

/// Start the gateway if it is not already answering, then wait for health.
pub fn ensure_api_running(app: &AppHandle) -> Result<ApiStatus, String> {
    let app_settings = settings::read_settings(app);
    let url = app_settings.api_url.trim_end_matches('/').to_string();
    let phone_url = crate::commands::advertised_api_url(Some(url.as_str()));
    let token = ensure_token(app)?;
    // The CLI-based tools (`list_peers`, `send_file_to_device`) read
    // ~/.config/adtp/cli.toml, so keep it on the exact same gateway + token.
    let _ = settings::write_cli_config(&token, &url);

    match probe(&url, &token) {
        Probe::Healthy => {
            let status = ApiStatus {
                running: true,
                url,
                phone_url,
                message: "Gateway already running".into(),
            };
            let _ = app.emit("api-status", status.clone());
            return Ok(status);
        }
        Probe::Unauthorized => {
            return Err(format!(
                "A gateway is already listening on {url} but rejects this desktop's pairing \
                 token. Stop it (systemctl --user stop adtp-api) or save the same token in \
                 Settings → Authentication."
            ));
        }
        Probe::Offline => {}
    }

    let (_, port, _) =
        parse_http_addr(&url).ok_or_else(|| format!("Invalid desktop API URL: {url}"))?;
    let _ = write_env_file(&token, port);

    let log_path = dirs::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("adtp")
        .join("gateway.log");
    if let Some(parent) = log_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let repo = find_repo_root();
    let candidates = launchers(repo.as_deref());
    let mut failures = Vec::new();
    let budget_deadline = Instant::now() + TOTAL_START_BUDGET;

    for launcher in candidates {
        if Instant::now() >= budget_deadline {
            failures.push(format!(
                "stopped after {} s while trying {}",
                TOTAL_START_BUDGET.as_secs(),
                launcher.label
            ));
            break;
        }

        let mut child = match spawn_detached(&launcher, &token, port, &log_path) {
            Ok(child) => child,
            Err(error) => {
                failures.push(format!("{} unavailable: {error}", launcher.label));
                continue;
            }
        };

        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            match probe(&url, &token) {
                Probe::Healthy => {
                    let status = ApiStatus {
                        running: true,
                        url,
                        phone_url,
                        message: format!("Gateway started via {}", launcher.label),
                    };
                    let _ = app.emit("api-status", status.clone());
                    return Ok(status);
                }
                Probe::Unauthorized => {
                    let _ = child.kill();
                    failures.push(format!(
                        "{} started but another process on port {port} uses a different token",
                        launcher.label
                    ));
                    break;
                }
                Probe::Offline => {}
            }

            // Fail fast when the candidate exits (missing module, port in use…)
            // instead of waiting the full timeout for a process that is gone.
            if let Ok(Some(status)) = child.try_wait() {
                failures.push(format!(
                    "{} exited with {status} (log: {})",
                    launcher.label,
                    log_path.display()
                ));
                break;
            }

            if Instant::now() >= deadline {
                let _ = child.kill();
                failures.push(format!(
                    "{} did not become healthy within {} s (log: {})",
                    launcher.label,
                    START_TIMEOUT.as_secs(),
                    log_path.display()
                ));
                break;
            }
            std::thread::sleep(Duration::from_millis(400));
        }
    }

    let message = format!(
        "Could not start the ADTP gateway. The app looks for an `adtp-api` binary next to \
         the desktop executable (packaged installs ship one), then on PATH and in a repo \
         checkout. Reinstall the latest ADTP Desktop package, set ADTP_API_BIN to a gateway \
         binary, or run scripts/install-listener.sh from a source checkout. Tried: {}. Log: {}",
        if failures.is_empty() {
            "nothing (no launcher available)".to_string()
        } else {
            failures.join(" | ")
        },
        log_path.display()
    );
    let status = ApiStatus {
        running: false,
        url,
        phone_url,
        message: message.clone(),
    };
    let _ = app.emit("api-status", status);
    Err(message)
}

#[tauri::command]
pub fn get_api_status(app: AppHandle) -> ApiStatus {
    let app_settings = settings::read_settings(&app);
    let url = app_settings.api_url.trim_end_matches('/').to_string();
    let phone_url = crate::commands::advertised_api_url(Some(url.as_str()));
    let token = settings::load_api_token(&app).unwrap_or_default();
    let probe = if token.is_empty() {
        Probe::Offline
    } else {
        probe(&url, &token)
    };
    ApiStatus {
        running: probe == Probe::Healthy,
        url,
        phone_url,
        message: match probe {
            Probe::Healthy => "Gateway online".into(),
            Probe::Unauthorized => "Gateway running with a different pairing token".into(),
            Probe::Offline => "Gateway offline".into(),
        },
    }
}

#[tauri::command]
pub async fn start_api_listener(app: AppHandle) -> Result<ApiStatus, String> {
    tauri::async_runtime::spawn_blocking(move || ensure_api_running(&app))
        .await
        .map_err(|error| error.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_http_urls() {
        assert_eq!(
            parse_http_addr("http://127.0.0.1:40003"),
            Some(("127.0.0.1".into(), 40003, false))
        );
        assert_eq!(
            parse_http_addr("https://desk.local"),
            Some(("desk.local".into(), 443, true))
        );
        assert_eq!(
            parse_http_addr("http://[::1]:9000/api"),
            Some(("::1".into(), 9000, false))
        );
    }

    #[test]
    fn rejects_non_http_urls() {
        assert_eq!(parse_http_addr("ws://127.0.0.1:40003"), None);
        assert_eq!(parse_http_addr("not a url"), None);
    }
}
