mod agent_manager;
mod commands;
mod p2p;
mod poller;
mod settings;
mod tray;

use adtp_peer_core::agent::ToolExecutor;
use agent_manager::AgentManager;
use p2p::P2PModule;
use poller::SharedState;
use std::sync::Arc;
use tauri::{Emitter, Listener, Manager};
use tokio::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let _ = rustls::crypto::ring::default_provider().install_default();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .plugin(tauri_plugin_shell::init())
        .manage::<SharedState>(Arc::new(Mutex::new(poller::AppState::default())))
        .manage::<Arc<Mutex<AgentManager>>>(Arc::new(Mutex::new(AgentManager::new())))
        .setup(|app| {
            tray::build_tray(app.handle())?;

            let config_dir = dirs::config_dir()
                .unwrap_or_else(|| std::path::PathBuf::from("."))
                .join("adtp");

            let p2p_rt = tokio::runtime::Runtime::new().expect("failed to create tokio runtime for P2P init");
            let p2p_option = p2p_rt.block_on(P2PModule::new(config_dir)).ok();
            if let Some(ref p2p) = p2p_option {
                tracing::info!("P2P module initialized — peer_id: {}", p2p.peer_id());
                let agent_mgr: Arc<Mutex<AgentManager>> = app.state::<Arc<Mutex<AgentManager>>>().inner().clone();
                let exec = p2p.clone() as Arc<dyn ToolExecutor>;
                p2p_rt.block_on(async {
                    let mgr = agent_mgr.lock().await;
                    mgr.set_tool_executor(exec).await;
                });
            } else {
                tracing::warn!("P2P module failed to initialize — agent file transfers will be unavailable");
            }
            app.manage::<Option<Arc<P2PModule>>>(p2p_option);
            std::mem::forget(p2p_rt); // keep runtime alive for background tasks (accept loop, etc.)

            let state: SharedState = app.state::<SharedState>().inner().clone();
            let handle = app.handle().clone();

            let handle_clone = handle.clone();
            let _ = handle.listen("notify", move |event| {
                use tauri_plugin_notification::NotificationExt;
                if let Ok(payload) = serde_json::from_str::<serde_json::Value>(event.payload()) {
                    let title = payload.get("title").and_then(|v| v.as_str()).unwrap_or("ADTP");
                    let body = payload.get("body").and_then(|v| v.as_str()).unwrap_or("");
                    if let Some(window) = handle_clone.get_webview_window("main") {
                        if window.is_visible().unwrap_or(true) {
                            return;
                        }
                    }
                    let _ = handle_clone.notification()
                        .builder()
                        .title(title)
                        .body(body)
                        .show();
                }
            });

            let handle_clone2 = handle.clone();
            let _ = handle.listen("health-update", move |event| {
                if let Ok(snapshot) = serde_json::from_str::<poller::HealthSnapshot>(event.payload()) {
                    let (state, text) = if snapshot.daemon_connected {
                        if snapshot.queue_pending > 0 || snapshot.queue_in_progress > 0 {
                            ("syncing", "Syncing")
                        } else {
                            ("idle", "Connected")
                        }
                    } else {
                        ("error", "Disconnected")
                    };
                    tray::update_tray_icon(&handle_clone2, state, text);
                }
            });

            let handle_clone3 = handle.clone();
            let _ = handle.listen("tray-send", move |_| {
                let h = handle_clone3.clone();
                tauri::async_runtime::spawn(async move {
                    use tauri_plugin_dialog::DialogExt;
                    if let Some(file) = h.dialog()
                        .file()
                        .add_filter("All Files", &["*"])
                        .blocking_pick_file()
                    {
                        if let Some(path) = file.as_path() {
                            let path_str = path.to_string_lossy().to_string();
                            let _ = h.emit("tray-send-path", path_str);
                        }
                    }
                });
            });

            poller::start_polling(handle.clone(), state);

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::send_file,
            commands::list_transfers,
            commands::get_transfer_status,
            commands::get_transfer_context,
            commands::cancel_transfer,
            commands::register_watch,
            commands::remove_watch,
            commands::list_watchers,
            commands::list_sessions,
            commands::get_audit_log,
            commands::verify_chain,
            commands::export_audit,
            commands::pick_file,
            commands::pick_save_folder,
            commands::save_settings,
            commands::load_settings,
            commands::save_api_token,
            commands::load_api_token,
            commands::write_file,
            commands::get_app_health,
            commands::agent_list,
            commands::agent_add,
            commands::agent_remove,
            commands::agent_toggle,
            commands::agent_send_message,
            commands::agent_get_permissions,
            commands::agent_get_conversation,
            commands::agent_clear_conversation,
            commands::get_home_dir,
            commands::get_own_peer_id,
            commands::list_peers,
            commands::send_file_to_peer,
            commands::add_peer,
            commands::remove_peer,
        ])
        .run(tauri::generate_context!())
        .expect("error while running ADTP Desktop");
}
