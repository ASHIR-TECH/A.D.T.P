use clap::Args;
use colored::Colorize;
use crate::config::CliConfig;
use crate::errors::CliError;

#[derive(Args)]
pub struct DesktopArgs {
    /// Path to the ADTP Desktop binary (default: auto-detect)
    #[arg(long)]
    pub binary: Option<String>,

    /// Launch in background (detach from terminal)
    #[arg(long, short)]
    pub background: bool,
}

pub async fn execute(cfg: &CliConfig, args: &DesktopArgs) -> Result<i32, CliError> {
    let binary = resolve_desktop_binary(args.binary.as_deref())?;

    if cfg.output_format == "json" {
        println!(r#"{{"launching": true, "binary": "{}"}}"#, binary.display());
    } else {
        println!("{} Launching ADTP Desktop...", "✓".green());
        println!("  Binary: {}", binary.display());
    }

    let status = std::process::Command::new(&binary)
        .spawn()
        .map_err(|e| CliError::Io {
            message: format!("Failed to launch ADTP Desktop at {}: {e}", binary.display()),
        })?;

    if args.background {
        // Detach — don't wait
        println!("  PID: {}", status.id());
        return Ok(0);
    }

    // Wait for the desktop process to exit
    let output = status.wait_with_output().map_err(|e| CliError::Io {
        message: format!("Error waiting for ADTP Desktop: {e}"),
    })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        eprintln!("ADTP Desktop exited with error: {stderr}");
        return Ok(1);
    }

    Ok(0)
}

fn resolve_desktop_binary(override_path: Option<&str>) -> Result<std::path::PathBuf, CliError> {
    if let Some(path) = override_path {
        return Ok(std::path::PathBuf::from(path));
    }

    // Search order: cwd, PATH, common install locations
    let candidates = [
        // Relative to workspace (development)
        "target/release/adtp-desktop",
        "target/debug/adtp-desktop",
        // Installed in PATH
        "adtp-desktop",
        // Linux AppImage
        "/usr/local/bin/adtp-desktop",
        "/usr/bin/adtp-desktop",
        // macOS
        "/Applications/ADTP Desktop.app/Contents/MacOS/adtp-desktop",
        // Windows
        r"C:\Program Files\ADTP Desktop\adtp-desktop.exe",
    ];

    for candidate in &candidates {
        let path = std::path::PathBuf::from(candidate);

        // If it's a bare name (no path separators), check PATH
        if !candidate.contains(std::path::MAIN_SEPARATOR) && !candidate.contains('/') {
            if which(candidate).is_some() {
                return Ok(path);
            }
        }

        if path.exists() {
            return Ok(path);
        }
    }

    Err(CliError::Config {
        message: "ADTP Desktop binary not found. Install it from https://ashir.world/adtp or use --binary <path>.".into(),
    })
}

fn which(name: &str) -> Option<std::path::PathBuf> {
    std::env::var_os("PATH").and_then(|paths| {
        std::env::split_paths(&paths).find_map(|dir| {
            let full = dir.join(name);
            if full.is_file() {
                Some(full)
            } else {
                None
            }
        })
    })
}
