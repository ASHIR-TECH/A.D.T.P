// Tauri-side relay listener manager
// Place in: desktop/src-tauri/src/relay_listener.rs
// Add to lib.rs: mod relay_listener;

use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::{AppHandle, Runtime};

/// Manages the Python relay listener as a background daemon.
/// Starts it on app launch, stops on app exit (optional — daemon survives if --daemon used).
pub struct RelayListenerManager {
    gateway_url: String,
    token: String,
    desktop_id: String,
    agent_socket: String,
    python_path: String,
    script_path: PathBuf,
    pid_file: PathBuf,
}

impl RelayListenerManager {
    pub fn new(
        gateway_url: String,
        token: String,
        desktop_id: String,
        agent_socket: Option<String>,
    ) -> Self {
        let script_path = Self::resolve_script_path();
        let pid_file = dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("adtp-relay-listener.pid");

        Self {
            gateway_url,
            token,
            desktop_id,
            agent_socket: agent_socket.unwrap_or_else(|| "/tmp/adtp-agent.sock".into()),
            python_path: "python3".into(),
            script_path,
            pid_file,
        }
    }

    fn resolve_script_path() -> PathBuf {
        // Try multiple locations: bundled resource, dev path, installed path
        let candidates = [
            // Bundled with Tauri (resources/relay_listener.py)
            std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.join("../resources/relay_listener.py"))),
            // Dev: repo root
            std::env::current_dir()
                .ok()
                .map(|p| p.join("relay_listener.py")),
            // Fallback
            Some(PathBuf::from("/usr/local/share/adtp/relay_listener.py")),
        ];

        for path in candidates.into_iter().flatten() {
            if path.exists() {
                return path;
            }
        }
        // Default fallback
        PathBuf::from("relay_listener.py")
    }

    /// Start the listener in daemon mode (forks, detaches, survives app close).
    pub fn start_daemon(&self) -> Result<(), String> {
        // Check if already running
        if self.is_running() {
            return Ok(());
        }

        let mut cmd = Command::new(&self.python_path);
        cmd.arg(&self.script_path)
            .arg("--daemon")
            .arg("--gateway-url")
            .arg(&self.gateway_url)
            .arg("--token")
            .arg(&self.token)
            .arg("--desktop-id")
            .arg(&self.desktop_id)
            .arg("--agent-socket")
            .arg(&self.agent_socket)
            .arg("--pid-file")
            .arg(&self.pid_file)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null());

        cmd.spawn()
            .map_err(|e| format!("Failed to start relay listener: {e}"))?;

        // Give it a moment to daemonize and write PID
        std::thread::sleep(std::time::Duration::from_millis(500));

        if !self.is_running() {
            return Err("Listener started but PID not found".into());
        }
        Ok(())
    }

    /// Start in foreground (for debugging / systemd). Blocks until stopped.
    #[allow(dead_code)]
    pub fn start_foreground(&self) -> Result<(), String> {
        let mut cmd = Command::new(&self.python_path);
        cmd.arg(&self.script_path)
            .arg("--gateway-url")
            .arg(&self.gateway_url)
            .arg("--token")
            .arg(&self.token)
            .arg("--desktop-id")
            .arg(&self.desktop_id)
            .arg("--agent-socket")
            .arg(&self.agent_socket);

        let mut child = cmd
            .spawn()
            .map_err(|e| format!("Failed to start relay listener: {e}"))?;

        child
            .wait()
            .map_err(|e| format!("Listener process error: {e}"))?;
        Ok(())
    }

    /// Stop the daemon if running.
    pub fn stop(&self) -> Result<(), String> {
        if let Some(pid) = self.read_pid() {
            #[cfg(unix)]
            {
                use std::process::Command;
                Command::new("kill")
                    .arg(pid.to_string())
                    .status()
                    .map_err(|e| format!("Failed to kill listener: {e}"))?;
            }
            #[cfg(windows)]
            {
                use std::process::Command;
                Command::new("taskkill")
                    .args(["/PID", &pid.to_string(), "/F"])
                    .status()
                    .map_err(|e| format!("Failed to kill listener: {e}"))?;
            }
            let _ = std::fs::remove_file(&self.pid_file);
        }
        Ok(())
    }

    /// Check if daemon is running (PID file exists and process alive).
    pub fn is_running(&self) -> bool {
        self.read_pid().is_some_and(Self::pid_alive)
    }

    fn read_pid(&self) -> Option<u32> {
        std::fs::read_to_string(&self.pid_file)
            .ok()
            .and_then(|s| s.trim().parse().ok())
    }

    #[cfg(unix)]
    fn pid_alive(pid: u32) -> bool {
        // Signal 0 checks existence without killing
        unsafe { libc::kill(pid as i32, 0) == 0 }
    }

    #[cfg(windows)]
    fn pid_alive(pid: u32) -> bool {
        use std::process::Command;
        Command::new("tasklist")
            .args(["/FI", &format!("PID eq {}", pid)])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).contains(&pid.to_string()))
            .unwrap_or(false)
    }
}

/// Tauri command: start the relay listener daemon.
#[tauri::command]
pub async fn start_relay_listener(
    _app: AppHandle,
    gateway_url: String,
    token: String,
    desktop_id: String,
    agent_socket: Option<String>,
) -> Result<(), String> {
    let manager = RelayListenerManager::new(gateway_url, token, desktop_id, agent_socket);
    manager.start_daemon()?;
    Ok(())
}

/// Tauri command: stop the relay listener daemon.
#[tauri::command]
pub async fn stop_relay_listener(
    _app: AppHandle,
    gateway_url: String,
    token: String,
    desktop_id: String,
    agent_socket: Option<String>,
) -> Result<(), String> {
    let manager = RelayListenerManager::new(gateway_url, token, desktop_id, agent_socket);
    manager.stop()?;
    Ok(())
}

/// Tauri command: check if listener is running.
#[tauri::command]
pub async fn relay_listener_status(
    _app: AppHandle,
    gateway_url: String,
    token: String,
    desktop_id: String,
    agent_socket: Option<String>,
) -> Result<bool, String> {
    let manager = RelayListenerManager::new(gateway_url, token, desktop_id, agent_socket);
    Ok(manager.is_running())
}

/// Auto-start on app launch: call from `setup()` or a `#[tauri::command]` invoked by frontend.
pub fn auto_start_on_launch<R: Runtime>(_app: &AppHandle<R>) -> Result<(), String> {
    // Read pairing from storage (same as pairingStorage)
    let data_dir = dirs::data_dir().unwrap_or_else(std::env::temp_dir);
    let pairing_file = data_dir.join("adtp-pairing.json");

    let pairing: Option<serde_json::Value> = std::fs::read_to_string(&pairing_file)
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok());

    if let Some(p) = pairing {
        let gateway_url = format!(
            "ws://{}:40003/relay",
            p["relayUrl"]
                .as_str()
                .unwrap_or("localhost")
                .replace("ws://", "")
                .replace("wss://", "")
                .split('/')
                .next()
                .unwrap_or("localhost")
        );
        let token = p["pairingToken"].as_str().unwrap_or("").to_string();
        let desktop_id = p["desktopId"].as_str().unwrap_or("").to_string();

        if !token.is_empty() && !desktop_id.is_empty() {
            let manager = RelayListenerManager::new(gateway_url, token, desktop_id, None);
            let _ = manager.start_daemon(); // Best effort
        }
    }
    Ok(())
}
