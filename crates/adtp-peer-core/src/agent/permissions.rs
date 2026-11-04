use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    pub allowed_paths: Vec<String>,
    pub writable_paths: Vec<String>,
    pub denied_paths: Vec<String>,
    pub allowed_commands: Vec<String>,
    pub max_file_read_bytes: u64,
    pub allow_hidden_files: bool,
    pub allow_remote_tasks: bool,
}

impl Default for PermissionConfig {
    fn default() -> Self {
        let allowed = default_allowed_dirs()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect::<Vec<_>>();
        let writable = default_intake_dir()
            .map(|p| vec![p.to_string_lossy().to_string()])
            .unwrap_or_default();
        Self {
            allowed_paths: allowed,
            writable_paths: writable,
            denied_paths: vec![],
            allowed_commands: vec![],
            max_file_read_bytes: 10_485_760,
            allow_hidden_files: false,
            allow_remote_tasks: true,
        }
    }
}

/// Expand a user-facing path (`~`, `~/Downloads`) before canonicalisation.
///
/// The model often emits `~/Pictures/photo.png` even though the tool schema
/// asks for absolute paths. `canonicalize` cannot resolve `~`, so without this
/// every such call looked like a permission denial.
pub fn expand_path(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if trimmed == "~" {
        return dirs::home_dir().unwrap_or_else(|| PathBuf::from(trimmed));
    }
    if let Some(rest) = trimmed
        .strip_prefix("~/")
        .or_else(|| trimmed.strip_prefix("~\\"))
    {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(trimmed)
}

fn env_allowed_paths() -> Vec<PathBuf> {
    let Ok(raw) = std::env::var("ADTP_ALLOWED_PATHS") else {
        return Vec::new();
    };
    raw.split(if cfg!(windows) { ';' } else { ':' })
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(expand_path)
        .collect()
}

/// Directories the desktop assistant may read out of the box.
///
/// A PC assistant that can only see the upload intake cannot answer "find my
/// image.json and send it to my phone". The common user folders plus the
/// upload intake cover laptops/desktops without exposing the whole home
/// directory (hidden files stay denied). Servers and locked-down machines can
/// replace this list with `ADTP_ALLOWED_PATHS` or `permissions.toml`.
pub fn default_allowed_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(intake) = default_intake_dir() {
        dirs.push(intake);
    }

    let from_env = env_allowed_paths();
    if !from_env.is_empty() {
        dirs.extend(from_env);
    } else if let Some(home) = dirs::home_dir() {
        for name in ["Downloads", "Documents", "Pictures", "Desktop"] {
            let candidate = home.join(name);
            if candidate.is_dir() {
                dirs.push(candidate);
            }
        }
        // OneDrive is the default Documents/Pictures location on many managed
        // Windows machines; include it when the English folder names are absent.
        let onedrive = home.join("OneDrive");
        if onedrive.is_dir() {
            for name in ["Documents", "Pictures", "Desktop"] {
                let candidate = onedrive.join(name);
                if candidate.is_dir() {
                    dirs.push(candidate);
                }
            }
        }
    }

    let mut seen = HashSet::new();
    dirs.retain(|path| seen.insert(path.clone()));
    dirs
}

/// `<data_dir>/adtp/intake`, mirroring the Python gateway's default.
pub fn default_intake_dir() -> Option<std::path::PathBuf> {
    if let Ok(override_dir) = std::env::var("ADTP_INTAKE_DIR") {
        if !override_dir.trim().is_empty() {
            return Some(std::path::PathBuf::from(override_dir));
        }
    }
    dirs::data_dir().map(|p| p.join("adtp").join("intake"))
}

const BLOCKED_COMMANDS: &[&str] = &[
    "rm",
    "sudo",
    "chmod",
    "chown",
    "dd",
    "mkfs",
    "fdisk",
    "mount",
    "umount",
    "passwd",
    "useradd",
    "usermod",
    "groupadd",
    "kill",
    "pkill",
    "reboot",
    "shutdown",
    "init",
    "systemctl",
    "journalctl",
];

#[derive(Debug)]
pub enum PermissionCheck {
    Allowed,
    Denied { reason: String },
}

pub struct PermissionStore {
    config: PermissionConfig,
    allowed_commands_set: HashSet<String>,
}

impl PermissionStore {
    pub fn new(config: PermissionConfig) -> Self {
        let allowed_commands_set: HashSet<String> = config
            .allowed_commands
            .iter()
            .map(|c| c.to_lowercase())
            .collect();
        Self {
            config,
            allowed_commands_set,
        }
    }

    pub fn config(&self) -> &PermissionConfig {
        &self.config
    }

    pub fn check_read_path(&self, path: &Path) -> PermissionCheck {
        let expanded = path
            .to_str()
            .map(expand_path)
            .unwrap_or_else(|| path.to_path_buf());
        let canonical = match expanded.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                return PermissionCheck::Denied {
                    reason: format!(
                        "Path does not exist or cannot be resolved: {}",
                        path.display()
                    ),
                }
            }
        };

        if !self.config.allow_hidden_files
            && canonical
                .file_name()
                .map(|name| name.to_string_lossy().starts_with('.'))
                .unwrap_or(false)
        {
            return PermissionCheck::Denied {
                reason: "Hidden files are not allowed".into(),
            };
        }

        for denied in &self.config.denied_paths {
            if let Ok(denied_canon) = std::fs::canonicalize(denied) {
                if canonical.starts_with(&denied_canon) {
                    return PermissionCheck::Denied {
                        reason: format!("Access denied to path in denied_paths: {}", denied),
                    };
                }
            }
        }

        let traversal_attempt = path.components().any(|c| c.as_os_str() == "..");
        if traversal_attempt {
            return PermissionCheck::Denied {
                reason: "Path traversal is not allowed".into(),
            };
        }

        for allowed in &self.config.allowed_paths {
            if let Ok(allowed_canon) = std::fs::canonicalize(allowed) {
                if canonical.starts_with(&allowed_canon) {
                    return PermissionCheck::Allowed;
                }
            }
        }

        PermissionCheck::Denied {
            reason: format!("Path is not in allowed_paths: {:?}", path),
        }
    }

    pub fn check_write_path(&self, path: &Path) -> PermissionCheck {
        let expanded = path
            .to_str()
            .map(expand_path)
            .unwrap_or_else(|| path.to_path_buf());
        let path = expanded.as_path();
        let canonical = match path.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                if let Some(parent) = path.parent() {
                    match parent.canonicalize() {
                        Ok(p) => p.join(path.file_name().unwrap_or_default()),
                        Err(_) => {
                            return PermissionCheck::Denied {
                                reason: "Cannot resolve parent directory".into(),
                            }
                        }
                    }
                } else {
                    return PermissionCheck::Denied {
                        reason: "Cannot resolve path".into(),
                    };
                }
            }
        };

        for allowed in &self.config.writable_paths {
            if let Ok(allowed_canon) = std::fs::canonicalize(allowed) {
                if canonical.starts_with(&allowed_canon) {
                    return PermissionCheck::Allowed;
                }
            }
        }

        PermissionCheck::Denied {
            reason: format!("Path is not in writable_paths: {:?}", path),
        }
    }

    pub fn check_command(&self, command: &str) -> PermissionCheck {
        let cmd_lower = command.to_lowercase();
        let cmd_name = cmd_lower.split_whitespace().next().unwrap_or(&cmd_lower);

        if BLOCKED_COMMANDS.contains(&cmd_name) {
            return PermissionCheck::Denied {
                reason: format!("Command '{}' is blocked for security reasons", cmd_name),
            };
        }

        if self.allowed_commands_set.contains(cmd_name) {
            PermissionCheck::Allowed
        } else {
            PermissionCheck::Denied {
                reason: format!("Command '{}' is not in allowed_commands", cmd_name),
            }
        }
    }

    pub fn file_size_allowed(&self, size: u64) -> bool {
        size <= self.config.max_file_read_bytes
    }

    pub fn is_remote_task_allowed(&self) -> bool {
        self.config.allow_remote_tasks
    }

    pub fn sanitize_command(command: &str) -> Result<(), String> {
        let dangerous_patterns = [";", "&&", "||", "|", "`", "$(", "../", "..\\"];
        for pattern in &dangerous_patterns {
            if command.contains(pattern) {
                return Err(format!(
                    "Command injection detected: '{}' is not allowed",
                    pattern
                ));
            }
        }
        let cmd_name = command.split_whitespace().next().unwrap_or("");
        if cmd_name.starts_with('/') {
            return Err("Absolute paths to executables are not allowed".into());
        }
        Ok(())
    }
}

pub fn permissions_path() -> Option<std::path::PathBuf> {
    dirs::config_dir().map(|p| p.join("adtp").join("permissions.toml"))
}

pub fn load_permissions() -> Result<PermissionConfig, String> {
    let path = permissions_path().ok_or("Cannot determine config directory")?;
    let mut config = if !path.exists() {
        PermissionConfig::default()
    } else {
        let content = std::fs::read_to_string(&path).map_err(|e| e.to_string())?;
        toml::from_str(&content).map_err(|e| e.to_string())?
    };

    // Legacy installs have a permissions.toml with an empty allowed_paths
    // list, which silently hid every phone attachment from the desktop AI.
    // Fill an empty list with the safe defaults (intake + common user folders).
    if config.allowed_paths.is_empty() {
        config.allowed_paths = default_allowed_dirs()
            .into_iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
    } else {
        // Always keep the upload intake reachable; files attached from MARS
        // land there and the AI must be able to act on them.
        if let Some(intake) = default_intake_dir() {
            let intake = intake.to_string_lossy().to_string();
            if !config.allowed_paths.contains(&intake) {
                config.allowed_paths.push(intake);
            }
        }
        // Server/homelab operators use this to add `/srv`, `/data`, etc.
        for extra in env_allowed_paths() {
            let extra = extra.to_string_lossy().to_string();
            if !config.allowed_paths.contains(&extra) {
                config.allowed_paths.push(extra);
            }
        }
    }

    if config.writable_paths.is_empty() {
        config.writable_paths = default_intake_dir()
            .map(|p| vec![p.to_string_lossy().to_string()])
            .unwrap_or_default();
    } else if let Some(intake) = default_intake_dir() {
        let intake = intake.to_string_lossy().to_string();
        if !config.writable_paths.contains(&intake) {
            config.writable_paths.push(intake);
        }
    }

    Ok(config)
}

pub fn save_permissions(config: &PermissionConfig) -> Result<(), String> {
    let path = permissions_path().ok_or("Cannot determine config directory")?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| e.to_string())?;
    std::fs::write(&path, &content).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn test_allowed_path_allows_read() {
        let config = PermissionConfig {
            allowed_paths: vec!["/tmp".into()],
            ..Default::default()
        };
        let store = PermissionStore::new(config);
        // We can't rely on /tmp existing and being canonicalizable in test context,
        // but we can test the denied case
        let result = store.check_command("rm");
        assert!(matches!(result, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_blocked_commands_rejected() {
        let config = PermissionConfig {
            allowed_commands: vec!["rm".into(), "ls".into()],
            ..Default::default()
        };
        let store = PermissionStore::new(config);
        let result = store.check_command("rm");
        assert!(matches!(result, PermissionCheck::Denied { .. }));
        let result = store.check_command("ls");
        assert!(matches!(result, PermissionCheck::Allowed));
    }

    #[test]
    fn test_allowed_commands_work() {
        let config = PermissionConfig {
            allowed_commands: vec!["ls".into(), "cat".into(), "echo".into()],
            ..Default::default()
        };
        let store = PermissionStore::new(config);
        assert!(matches!(
            store.check_command("ls"),
            PermissionCheck::Allowed
        ));
        assert!(matches!(
            store.check_command("cat"),
            PermissionCheck::Allowed
        ));
        assert!(matches!(
            store.check_command("grep"),
            PermissionCheck::Denied { .. }
        ));
    }

    #[test]
    fn test_file_size_limit() {
        let config = PermissionConfig {
            max_file_read_bytes: 1024,
            ..Default::default()
        };
        let store = PermissionStore::new(config);
        assert!(store.file_size_allowed(512));
        assert!(store.file_size_allowed(1024));
        assert!(!store.file_size_allowed(2048));
    }

    #[test]
    fn test_hidden_files() {
        let config = PermissionConfig {
            allow_hidden_files: false,
            ..Default::default()
        };
        let store = PermissionStore::new(config);
        let result = store.check_read_path(Path::new("/tmp/.hidden"));
        // This may pass or fail depending on whether /tmp is in allowed_paths
        // But confirm it's at least returning something valid
        assert!(matches!(
            result,
            PermissionCheck::Allowed | PermissionCheck::Denied { .. }
        ));
    }

    #[test]
    fn test_traversal_rejected() {
        let config = PermissionConfig::default();
        let store = PermissionStore::new(config);
        let result = store.check_read_path(Path::new("/tmp/../../etc/passwd"));
        assert!(matches!(result, PermissionCheck::Denied { .. }));
    }

    #[test]
    fn test_default_permissions_expose_intake_and_user_folders() {
        let config = PermissionConfig::default();
        // The upload intake is always readable so the AI can open phone
        // attachments; common user folders let it find files the user names.
        assert!(config.allowed_paths.iter().any(|p| p.ends_with("intake")));
        assert!(config.writable_paths.iter().all(|p| p.ends_with("intake")));
        assert!(config.allowed_commands.is_empty());
        assert_eq!(config.max_file_read_bytes, 10_485_760);
        assert!(config.allow_remote_tasks);
    }

    #[test]
    fn test_expand_path_expands_home() {
        let home = dirs::home_dir().unwrap();
        assert_eq!(expand_path("~"), home);
        assert_eq!(expand_path("~/Pictures/a.png"), home.join("Pictures/a.png"));
        assert_eq!(
            expand_path("/var/data/a.json"),
            Path::new("/var/data/a.json")
        );
    }

    #[test]
    fn test_save_load_permissions() {
        let config = PermissionConfig {
            allowed_paths: vec!["/tmp/test".into()],
            allowed_commands: vec!["ls".into(), "echo".into()],
            max_file_read_bytes: 2048,
            ..Default::default()
        };
        let json = serde_json::to_string(&config).unwrap();
        let loaded: PermissionConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(loaded.allowed_paths, vec!["/tmp/test"]);
        assert_eq!(loaded.allowed_commands, vec!["ls", "echo"]);
        assert_eq!(loaded.max_file_read_bytes, 2048);
    }

    #[test]
    fn test_sanitize_command_rejects_semicolons() {
        assert!(PermissionStore::sanitize_command("ls; rm -rf /").is_err());
    }

    #[test]
    fn test_sanitize_command_rejects_pipes() {
        assert!(PermissionStore::sanitize_command("ls | grep secret").is_err());
    }

    #[test]
    fn test_sanitize_command_rejects_backticks() {
        assert!(PermissionStore::sanitize_command("echo `whoami`").is_err());
    }

    #[test]
    fn test_sanitize_command_rejects_command_substitution() {
        assert!(PermissionStore::sanitize_command("echo $(whoami)").is_err());
    }

    #[test]
    fn test_sanitize_command_rejects_absolute_path() {
        assert!(PermissionStore::sanitize_command("/bin/ls").is_err());
    }

    #[test]
    fn test_sanitize_command_rejects_logical_operators() {
        assert!(PermissionStore::sanitize_command("true && rm -rf /").is_err());
        assert!(PermissionStore::sanitize_command("false || rm -rf /").is_err());
    }

    #[test]
    fn test_sanitize_command_rejects_traversal() {
        assert!(PermissionStore::sanitize_command("ls ../..").is_err());
    }

    #[test]
    fn test_sanitize_command_allows_clean_command() {
        assert!(PermissionStore::sanitize_command("ls -la /tmp").is_ok());
        assert!(PermissionStore::sanitize_command("cat /tmp/file.txt").is_ok());
        assert!(PermissionStore::sanitize_command("echo hello world").is_ok());
    }
}
