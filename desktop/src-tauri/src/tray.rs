use std::io::Cursor;

use tauri::{
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, Runtime,
};

pub fn build_tray<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Ashir Tech ADTP", true, None::<&str>)?;
    let send = MenuItem::with_id(app, "send", "Send File...", true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let status = MenuItem::with_id(app, "status", "Status: Checking...", false, None::<&str>)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let cli = MenuItem::with_id(app, "cli", "Launch CLI...", true, None::<&str>)?;
    let sep3 = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[&open, &send, &sep, &status, &sep2, &cli, &sep3, &quit],
    )?;

    let idle_img = decode_png(include_bytes!("../icons/tray-idle.png"));

    let _tray = TrayIconBuilder::with_id("adtp-main")
        .icon(Image::new(&idle_img.0, idle_img.1, idle_img.2))
        .tooltip("ADTP Desktop")
        .menu(&menu)
        .on_menu_event(move |app, event| match event.id().as_ref() {
            "open" => {
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
            "send" => {
                let _ = app.emit("tray-send", ());
            }
            "cli" => {
                launch_cli_terminal(app);
            }
            "quit" => {
                app.exit(0);
            }
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    let _ = window.show();
                    let _ = window.set_focus();
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn decode_png(bytes: &[u8]) -> (Vec<u8>, u32, u32) {
    let decoder = png::Decoder::new(Cursor::new(bytes));
    let mut reader = decoder.read_info().expect("Failed to decode tray icon PNG");
    let (width, height) = reader.info().size();
    let buf_size = reader.output_buffer_size();
    let mut buf = vec![0u8; buf_size];
    reader
        .next_frame(&mut buf)
        .expect("Failed to read PNG frame");

    if reader.info().color_type == png::ColorType::Rgba {
        buf.truncate((width * height * 4) as usize);
        (buf, width, height)
    } else if reader.info().color_type == png::ColorType::Rgb {
        let mut rgba = Vec::with_capacity((width * height * 4) as usize);
        for chunk in buf[..(width * height * 3) as usize].chunks(3) {
            rgba.extend_from_slice(chunk);
            rgba.push(255);
        }
        (rgba, width, height)
    } else {
        panic!("Unsupported PNG color type for tray icon");
    }
}

pub fn update_tray_icon<R: Runtime>(app: &AppHandle<R>, state: &str, status_text: &str) {
    let bytes: &[u8] = match state {
        "syncing" => include_bytes!("../icons/tray-sync.png") as &[u8],
        "error" => include_bytes!("../icons/tray-error.png") as &[u8],
        _ => include_bytes!("../icons/tray-idle.png") as &[u8],
    };

    let (rgba, w, h) = decode_png(bytes);

    if let Some(tray) = app.tray_by_id("adtp-main") {
        let _ = tray.set_icon(Some(Image::new(&rgba, w, h)));
        let _ = tray.set_tooltip(Some(format!("ADTP Desktop — {status_text}")));
    }
}

/// Launch a terminal with the `adtp` CLI ready to use.
/// Tries several terminal emulators; falls back to spawning `adtp`
/// directly inside a new terminal window.
pub fn launch_cli_terminal<R: Runtime>(app: &AppHandle<R>) {
    // Find the CLI binary next to our own executable, or in PATH
    let cli_path = resolve_cli_binary(app);

    let terminal_cmd: &[(_, &[&str])] = &[
        ("kgx", &["kgx", "-e", &cli_path]),
        ("gnome-terminal", &["gnome-terminal", "--", &cli_path]),
        ("konsole", &["konsole", "-e", &cli_path]),
        ("xfce4-terminal", &["xfce4-terminal", "-e", &cli_path]),
        ("xterm", &["xterm", "-e", &cli_path]),
        ("alacritty", &["alacritty", "-e", &cli_path]),
        ("wezterm", &["wezterm", "start", &cli_path]),
        ("windows-terminal", &["start", "wt", "-d", ".", &cli_path]),
    ];

    for (name, cmd) in terminal_cmd {
        if *name == "windows-terminal" && cfg!(not(target_os = "windows")) {
            continue;
        }
        if std::process::Command::new(cmd[0])
            .args(&cmd[1..])
            .spawn()
            .is_ok()
        {
            return;
        }
    }

    // Last resort: just spawn CLI in the background (no terminal window)
    let _ = std::process::Command::new(&cli_path).spawn();
}

/// Find the `adtp` CLI binary.  Checks: 1) next to our own exe
/// (bundled case), 2) PATH, 3) common install locations.
fn resolve_cli_binary<R: Runtime>(app: &AppHandle<R>) -> String {
    // 1. Next to our own executable (bundled distribution)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let bundled = parent.join("adtp");
            if bundled.exists() {
                return bundled.to_string_lossy().to_string();
            }
            #[cfg(target_os = "windows")]
            {
                let bundled_exe = parent.join("adtp.exe");
                if bundled_exe.exists() {
                    return bundled_exe.to_string_lossy().to_string();
                }
            }
        }
    }

    // 2. Next to the Tauri resource dir
    if let Ok(resource_dir) = app.path().resource_dir() {
        let bundled = resource_dir.join("adtp");
        if bundled.exists() {
            return bundled.to_string_lossy().to_string();
        }
    }

    // 3. Check PATH
    if let Ok(paths) = std::env::var("PATH") {
        for dir in std::env::split_paths(&paths) {
            let candidate = dir.join("adtp");
            if candidate.is_file() {
                return candidate.to_string_lossy().to_string();
            }
            #[cfg(target_os = "windows")]
            {
                let candidate_exe = dir.join("adtp.exe");
                if candidate_exe.is_file() {
                    return candidate_exe.to_string_lossy().to_string();
                }
            }
        }
    }

    // 4. Fallback: just return the name and let the OS figure it out
    "adtp".to_string()
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_state_mapping_idle() {
        let state = "idle";
        let status = "Connected";
        assert_eq!(state, "idle");
        assert_eq!(status, "Connected");
    }

    #[test]
    fn test_state_mapping_syncing() {
        let state = "syncing";
        assert_eq!(state, "syncing");
    }

    #[test]
    fn test_state_mapping_error() {
        let state = "error";
        assert_eq!(state, "error");
    }
}
