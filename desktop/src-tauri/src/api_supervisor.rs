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

use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
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

/// Persisted record of the gateway this user's desktop started. Keeping the
/// *URL* (not just a PID) is what lets a later launch adopt a gateway that
/// ended up on an ephemeral port instead of blindly spawning another one.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
struct GatewayRecord {
    pid: u32,
    /// Process-group / session id. `spawn_detached` calls `setsid()`, so the
    /// child leads its own group; this is what lets us reap PyInstaller's
    /// bootloader **and** the Python child it forks.
    #[serde(default)]
    pgid: Option<i32>,
    url: String,
}

fn adtp_data_dir() -> Option<PathBuf> {
    // Overridable for portable installs and tests; otherwise the same XDG
    // location the rest of the app uses.
    if let Some(dir) = std::env::var_os("ADTP_DATA_DIR") {
        return Some(PathBuf::from(dir));
    }
    dirs::data_dir().map(|d| d.join("adtp"))
}

fn gateway_record_path() -> Option<PathBuf> {
    adtp_data_dir().map(|d| d.join("gateway.json"))
}

/// Legacy PID file written by earlier builds. Removed once a record exists.
fn legacy_pid_path() -> Option<PathBuf> {
    adtp_data_dir().map(|d| d.join("gateway.pid"))
}

fn gateway_lock_path() -> Option<PathBuf> {
    adtp_data_dir().map(|d| d.join("gateway.lock"))
}

fn read_gateway_record() -> Option<GatewayRecord> {
    let raw = fs::read_to_string(gateway_record_path()?).ok()?;
    let record: GatewayRecord = serde_json::from_str(&raw).ok()?;
    if record.url.is_empty() {
        return None;
    }
    Some(record)
}

/// Atomic write: a half-written record must never make the next launch think
/// the gateway is broken and start a second one.
fn write_gateway_record(record: &GatewayRecord) {
    let Some(path) = gateway_record_path() else {
        return;
    };
    let Some(dir) = path.parent() else {
        return;
    };
    let _ = fs::create_dir_all(dir);
    let Ok(bytes) = serde_json::to_vec(record) else {
        return;
    };
    let tmp = path.with_extension("json.tmp");
    if fs::write(&tmp, bytes).is_ok() && fs::rename(&tmp, &path).is_err() {
        let _ = fs::remove_file(&tmp);
    }
}

fn clear_gateway_record() {
    if let Some(path) = gateway_record_path() {
        let _ = fs::remove_file(path);
    }
    if let Some(path) = legacy_pid_path() {
        let _ = fs::remove_file(path);
    }
}

/// Is this process still around?
fn pid_alive(pid: u32) -> bool {
    if pid <= 1 {
        return false;
    }
    #[cfg(unix)]
    {
        Path::new(&format!("/proc/{pid}")).exists()
    }
    #[cfg(not(unix))]
    {
        // Best effort: Windows has no /proc. The HTTP probe is the real
        // liveness check; this only guards stale cleanup.
        let _ = pid;
        true
    }
}

/// Ask the gateway's whole process group to stop, then force it after a short
/// grace period. Killing only `Child::kill()` (the PyInstaller bootloader)
/// leaves the actual Python listener running as an orphan — that is exactly
/// how a single launch used to accumulate 20+ `adtp-api` processes.
fn terminate_gateway(record: &GatewayRecord) {
    #[cfg(unix)]
    {
        if record.pid <= 1 {
            return;
        }
        let pgid = record.pgid.unwrap_or(record.pid as i32);
        if pgid > 1 {
            unsafe {
                libc::kill(-pgid, libc::SIGTERM);
            }
            for _ in 0..20 {
                if !pid_alive(record.pid) {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            unsafe {
                libc::kill(-pgid, libc::SIGKILL);
            }
        }
        if record.pid > 1 && pid_alive(record.pid) {
            unsafe {
                libc::kill(record.pid as i32, libc::SIGKILL);
            }
        }
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/PID", &record.pid.to_string(), "/T", "/F"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
}

/// Cross-process mutex around the "probe → spawn → record" sequence.
///
/// Without this, the startup task, the QR panel and the Settings button (each
/// in its own desktop process) all saw "no gateway" at the same time and each
/// spawned one. On Unix this is an advisory `flock`, which the kernel releases
/// even if the owner crashes; on Windows it is an atomic exclusive-create file
/// with a stale-owner check.
struct GatewayStartLock {
    path: PathBuf,
    #[cfg(unix)]
    file: fs::File,
}

impl GatewayStartLock {
    fn acquire(timeout: Duration) -> Option<Self> {
        let path = gateway_lock_path()?;
        if let Some(dir) = path.parent() {
            let _ = fs::create_dir_all(dir);
        }

        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let file = fs::OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(&path)
                .ok()?;
            let deadline = Instant::now() + timeout;
            loop {
                let rc = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
                if rc == 0 {
                    return Some(Self { path, file });
                }
                let error = std::io::Error::last_os_error();
                let would_block = matches!(
                    error.raw_os_error(),
                    Some(code)
                        if code == libc::EWOULDBLOCK
                            || code == libc::EAGAIN
                            || code == libc::EINTR
                );
                if !would_block || Instant::now() >= deadline {
                    return None;
                }
                std::thread::sleep(Duration::from_millis(150));
            }
        }

        #[cfg(not(unix))]
        {
            let deadline = Instant::now() + timeout;
            loop {
                match fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&path)
                {
                    Ok(mut file) => {
                        let _ = writeln!(file, "{}", std::process::id());
                        return Some(Self { path });
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        if lock_is_stale(&path) {
                            let _ = fs::remove_file(&path);
                            continue;
                        }
                        if Instant::now() >= deadline {
                            return None;
                        }
                        std::thread::sleep(Duration::from_millis(150));
                    }
                    Err(_) => return None,
                }
            }
        }
    }
}

#[cfg(not(unix))]
fn lock_is_stale(path: &Path) -> bool {
    let holder = fs::read_to_string(path)
        .ok()
        .and_then(|contents| contents.trim().parse::<u32>().ok());
    if let Some(pid) = holder {
        if !pid_alive(pid) {
            return true;
        }
    }
    let old = fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| modified.elapsed().ok())
        .map(|age| age > Duration::from_secs(180))
        .unwrap_or(false);
    old
}

impl Drop for GatewayStartLock {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            use std::os::unix::io::AsRawFd;
            let _ = unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
            let _ = &self.path;
        }
        #[cfg(not(unix))]
        {
            let _ = fs::remove_file(&self.path);
        }
    }
}

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

/// Can we bind `host:port` right now? Used to detect a port that is already
/// taken before we ask the gateway to bind it (uvicorn will just exit with an
/// "address already in use" error instead of picking another port).
fn port_available(host: &str, port: u16) -> bool {
    (host, port)
        .to_socket_addrs()
        .ok()
        .and_then(|addrs| {
            addrs
                .into_iter()
                .find_map(|addr| TcpListener::bind(addr).ok())
        })
        .is_some()
}

/// Pick a port the OS considers free right now. Bind to port 0, read back the
/// assigned ephemeral port, then release it so the gateway can bind it. There
/// is a tiny race window (another process could grab it in between), but it is
/// good enough to escape a busy default port.
fn find_free_port(host: &str) -> Option<u16> {
    let addr = (host, 0).to_socket_addrs().ok()?.next()?;
    let listener = TcpListener::bind(addr).ok()?;
    let port = listener.local_addr().ok()?.port();
    drop(listener);
    Some(port)
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

/// A single missed probe must not make us kill a perfectly healthy gateway.
/// Under load (or right after the OS resumes) one TCP/TLS attempt can time
/// out; retrying briefly avoids a needless restart and the process churn that
/// comes with it.
fn probe_retrying(url: &str, token: &str, attempts: u32) -> Probe {
    let mut last = Probe::Offline;
    for attempt in 0..attempts.max(1) {
        last = probe(url, token);
        if last != Probe::Offline {
            return last;
        }
        if attempt + 1 < attempts {
            std::thread::sleep(Duration::from_millis(250));
        }
    }
    last
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

/// Resolve a launcher program to a stable identity so the same binary reached
/// via `PATH`, an absolute path and a symlink is only tried once.
fn resolve_program(program: &str) -> PathBuf {
    let path = Path::new(program);
    if path.is_absolute() || program.contains(std::path::MAIN_SEPARATOR) {
        return fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join(program);
            if candidate.is_file() {
                return fs::canonicalize(&candidate).unwrap_or(candidate);
            }
        }
    }
    PathBuf::from(program)
}

fn push_unique(out: &mut Vec<Launcher>, launcher: Launcher) {
    let key = resolve_program(&launcher.program);
    let duplicate = out.iter().any(|existing| {
        resolve_program(&existing.program) == key && existing.args == launcher.args
    });
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
            // `--onedir` layout: the launcher lives inside the folder.
            paths.push(dir.join("adtp-api").join(name));
        }
        for sub in ["resources", "binaries"] {
            for name in ["adtp-api", "adtp-api.exe"] {
                paths.push(dir.join(sub).join(name));
                paths.push(dir.join(sub).join("adtp-api").join(name));
            }
        }
    }

    if let Some(data) = dirs::data_dir() {
        let adtp = data.join("adtp");
        paths.push(adtp.join("adtp-api"));
        paths.push(adtp.join("adtp-api.exe"));
        paths.push(adtp.join("adtp-api").join("adtp-api"));
        paths.push(adtp.join("adtp-api").join("adtp-api.exe"));
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
        "/usr/lib/adtp/adtp-api/adtp-api",
        "/opt/adtp/bin/adtp-api",
        "/opt/adtp/adtp-api/adtp-api",
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
        // Both the package-specific environment (`uv sync` in packages/adtp-api)
        // and the legacy repo-root one are valid; try whichever is healthy.
        let venvs = [
            repo.join("packages").join("adtp-api").join(".venv"),
            repo.join(".venv"),
        ];
        for venv in &venvs {
            for name in [
                "python3.14",
                "python3.13",
                "python3.12",
                "python3.11",
                "python",
            ] {
                let python = venv.join("bin").join(name);
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
        // Keep the idle footprint small: glibc otherwise creates one malloc
        // arena per thread (uvicorn + aiosqlite + pydantic can spawn several),
        // each reserving tens of MB of virtual address space.
        .env("MALLOC_ARENA_MAX", "2")
        .env("PYTHONUNBUFFERED", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
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

/// Point settings, the CLI config and `api.env` at the gateway we are using so
/// the QR, the agent tools and the next launch all agree on the same URL.
fn adopt_running(
    app: &AppHandle,
    url: String,
    token: &str,
    message: impl Into<String>,
) -> ApiStatus {
    let app_settings = settings::read_settings(app);
    if app_settings.api_url.trim_end_matches('/') != url {
        let mut updated = app_settings;
        updated.api_url = url.clone();
        let _ = settings::write_settings(app, &updated);
    }
    if let Some((_, port, _)) = parse_http_addr(&url) {
        let _ = write_env_file(token, port);
    }
    let _ = settings::write_cli_config(token, &url);
    if let Some(legacy) = legacy_pid_path() {
        let _ = fs::remove_file(legacy);
    }
    let phone_url = crate::commands::advertised_api_url(Some(url.as_str()));
    ApiStatus {
        running: true,
        url,
        phone_url,
        message: message.into(),
    }
}

/// Start the gateway if it is not already answering, then wait for health.
///
/// Single-owner invariant: at most one gateway is started per user. A launch
/// first *adopts* whatever is already healthy (recorded URL, then configured
/// URL); only if nothing answers does it take a cross-process lock and spawn
/// exactly one candidate, reaping the whole process group of every failed
/// candidate before trying the next launcher.
pub fn ensure_api_running(app: &AppHandle) -> Result<ApiStatus, String> {
    let app_settings = settings::read_settings(app);
    let configured_url = app_settings.api_url.trim_end_matches('/').to_string();
    let token = ensure_token(app)?;
    // The CLI-based tools (`list_peers`, `send_file_to_device`) read
    // ~/.config/adtp/cli.toml, so keep it on the exact same gateway + token.
    let _ = settings::write_cli_config(&token, &configured_url);

    let (host, configured_port, _) = parse_http_addr(&configured_url)
        .ok_or_else(|| format!("Invalid desktop API URL: {configured_url}"))?;

    // 1. Adopt the gateway recorded by a previous launch, whatever port it
    //    chose. This is the normal path after the first run, and it is why a
    //    desktop restart no longer leaves a second gateway behind.
    if let Some(record) = read_gateway_record() {
        match probe_retrying(&record.url, &token, 3) {
            Probe::Healthy => {
                let status = adopt_running(app, record.url, &token, "Gateway already running");
                let _ = app.emit("api-status", status.clone());
                return Ok(status);
            }
            Probe::Unauthorized => {
                // Our token rotated; this gateway can never serve us.
                terminate_gateway(&record);
                clear_gateway_record();
            }
            Probe::Offline => {
                // Hung or half-started leftover from an older build. Reap the
                // whole group before starting a replacement.
                if pid_alive(record.pid) {
                    terminate_gateway(&record);
                }
                clear_gateway_record();
            }
        }
    }

    // 2. A listener we did not spawn (systemd, pipx, an older build) may still
    //    be running on the configured URL.
    match probe(&configured_url, &token) {
        Probe::Healthy => {
            let status = adopt_running(
                app,
                configured_url.clone(),
                &token,
                "Gateway already running",
            );
            let _ = app.emit("api-status", status.clone());
            return Ok(status);
        }
        Probe::Unauthorized => {
            return Err(format!(
                "A gateway is already listening on {configured_url} but rejects this desktop's \
                 pairing token. Stop it (systemctl --user stop adtp-api) or save the same token \
                 in Settings → Authentication."
            ));
        }
        Probe::Offline => {}
    }

    // 3. Serialize startup across desktop processes and concurrent commands.
    //    Without the lock, `setup`, the QR panel and the Settings button each
    //    saw "offline" and each spawned a gateway.
    let start_lock = GatewayStartLock::acquire(Duration::from_secs(120));

    // Another process may have finished starting while we waited. Re-check
    // under the lock before spawning anything.
    if let Some(record) = read_gateway_record() {
        match probe_retrying(&record.url, &token, 3) {
            Probe::Healthy => {
                let status = adopt_running(app, record.url, &token, "Gateway already running");
                let _ = app.emit("api-status", status.clone());
                return Ok(status);
            }
            Probe::Unauthorized | Probe::Offline => {
                if pid_alive(record.pid) {
                    terminate_gateway(&record);
                }
                clear_gateway_record();
            }
        }
    }
    if let Probe::Healthy = probe(&configured_url, &token) {
        let status = adopt_running(
            app,
            configured_url.clone(),
            &token,
            "Gateway already running",
        );
        let _ = app.emit("api-status", status.clone());
        return Ok(status);
    }

    if start_lock.is_none() {
        return Err(
            "Another ADTP process is still starting the gateway. Try again in a moment.".into(),
        );
    }

    // Pick a port the gateway can actually bind. If the configured port is
    // taken (firewall, another process), fall back to an ephemeral port so the
    // phone can still reach us over the LAN.
    let port = if port_available(&host, configured_port) {
        configured_port
    } else {
        find_free_port(&host).ok_or_else(|| {
            format!("Port {configured_port} is in use and no ephemeral port is available")
        })?
    };

    let url = format!("http://{host}:{port}");

    // Persist the resolved port so the phone's QR, the CLI config and the next
    // launch all agree with the gateway that is actually running.
    if port != configured_port {
        let mut updated = app_settings;
        updated.api_url = url.clone();
        let _ = settings::write_settings(app, &updated);
    }
    let _ = write_env_file(&token, port);
    let phone_url = crate::commands::advertised_api_url(Some(url.as_str()));

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

        let pid = child.id();
        #[cfg(unix)]
        let pgid = Some(pid as i32);
        #[cfg(not(unix))]
        let pgid = None;

        // Every failed attempt must be reaped as a *group*: PyInstaller's
        // one-file bootloader forks the real Python listener, so `child.kill()`
        // alone would leave an orphan holding the port and the DB.
        let deadline = Instant::now() + START_TIMEOUT;
        loop {
            match probe(&url, &token) {
                Probe::Healthy => {
                    write_gateway_record(&GatewayRecord {
                        pid,
                        pgid,
                        url: url.clone(),
                    });
                    if let Some(legacy) = legacy_pid_path() {
                        let _ = fs::remove_file(legacy);
                    }
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

        // Reaching here means this candidate never became healthy. Reap the
        // whole group before the next launcher starts, so a bad attempt can
        // never leave a listener (or a bootloader) behind.
        terminate_gateway(&GatewayRecord {
            pid,
            pgid,
            url: url.clone(),
        });
        let _ = child.kill();
        let _ = child.wait();
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

    #[test]
    fn gateway_record_roundtrips_with_optional_pgid() {
        let record = GatewayRecord {
            pid: 4242,
            pgid: Some(4242),
            url: "http://127.0.0.1:41000".into(),
        };
        let json = serde_json::to_string(&record).expect("serialize");
        let back: GatewayRecord = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back.pid, 4242);
        assert_eq!(back.pgid, Some(4242));
        assert_eq!(back.url, record.url);

        // Older records may not carry a pgid; it must default to None.
        let legacy: GatewayRecord =
            serde_json::from_str(r#"{"pid":7,"url":"http://127.0.0.1:40003"}"#).expect("legacy");
        assert_eq!(legacy.pgid, None);
    }

    #[test]
    fn push_unique_skips_duplicate_launchers() {
        let mut out = Vec::new();
        push_unique(&mut out, Launcher::new("adtp-api"));
        push_unique(&mut out, Launcher::new("adtp-api"));
        assert_eq!(out.len(), 1);

        push_unique(
            &mut out,
            Launcher::new("adtp-api").args(vec!["--flag".into()]),
        );
        assert_eq!(out.len(), 2, "different args are a different launcher");
    }

    #[test]
    fn terminating_a_bogus_pid_is_a_noop() {
        // pgid <= 1 would signal our own process group; the guard must prevent
        // that. This test would kill the test runner if the guard regressed.
        terminate_gateway(&GatewayRecord {
            pid: 0,
            pgid: Some(0),
            url: "http://127.0.0.1:40003".into(),
        });
        terminate_gateway(&GatewayRecord {
            pid: 1,
            pgid: Some(1),
            url: "http://127.0.0.1:40003".into(),
        });
    }

    #[test]
    fn start_lock_serializes_concurrent_starters() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::env::set_var("ADTP_DATA_DIR", dir.path());

        let first = GatewayStartLock::acquire(Duration::from_secs(2)).expect("first lock");
        let handle = std::thread::spawn(|| {
            let start = Instant::now();
            let second = GatewayStartLock::acquire(Duration::from_secs(5)).expect("second lock");
            (start.elapsed(), second)
        });

        // Give the second starter time to block on the lock, then release it.
        std::thread::sleep(Duration::from_millis(400));
        drop(first);

        let (waited, second) = handle.join().expect("join");
        assert!(
            waited >= Duration::from_millis(200),
            "second starter did not wait for the lock (waited {waited:?})"
        );
        drop(second);
        std::env::remove_var("ADTP_DATA_DIR");
    }
}
