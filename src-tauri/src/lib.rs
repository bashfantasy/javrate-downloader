mod auto_relay;
mod cdn_adapter;
mod commands;
mod download_engine;
mod extraction;
mod task_management;

use download_engine::{ProgressPayload, RelayNeededPayload, StateChangedPayload};
use tauri::{Listener, Manager};

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_opener::init())
        .manage(task_management::TaskStore::default())
        .manage(download_engine::DownloadProcessRegistry::default())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                let path = std::env::var("PATH").unwrap_or_else(|_| "".to_string());
                let new_path = format!("{}:/opt/homebrew/bin:/usr/local/bin", path);
                std::env::set_var("PATH", new_path);
            }
            let store = app.state::<task_management::TaskStore>();
            let storage_path = dirs::data_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
                .join("javrate-downloader")
                .join("tasks.json");
            if let Err(err) = store
                .configure_storage(storage_path)
                .and_then(|_| store.load_from_disk())
            {
                eprintln!("failed to initialize task persistence: {err}");
            }

            let handle = app.handle().clone();
            // NOTE: per-task 接力鎖，確保同一任務同時只有一個接力流程在執行
            let relay_in_progress: std::sync::Arc<std::sync::Mutex<std::collections::HashSet<String>>> =
                std::sync::Arc::new(std::sync::Mutex::new(std::collections::HashSet::new()));
            app.listen("relay-needed", move |event| {
                let Ok(payload) = serde_json::from_str::<RelayNeededPayload>(event.payload())
                else {
                    return;
                };

                // 如果這個任務已經有接力流程在跑，直接忽略
                {
                    let lock = relay_in_progress.lock().expect("relay lock poisoned");
                    if lock.contains(&payload.task_id) {
                        return;
                    }
                }
                // 標記為「接力中」
                {
                    let mut lock = relay_in_progress.lock().expect("relay lock poisoned");
                    lock.insert(payload.task_id.clone());
                }

                let app = handle.clone();
                let relay_lock = relay_in_progress.clone();
                let task_id = payload.task_id.clone();
                tauri::async_runtime::spawn(async move {
                    let store = app.state::<task_management::TaskStore>();
                    let processes = app.state::<download_engine::DownloadProcessRegistry>();
                    if let Err(err) = auto_relay::handle_relay_needed(
                        store.inner(),
                        processes.inner(),
                        app.clone(),
                        &task_id,
                    )
                    .await
                    {
                        eprintln!("auto relay failed for task {}: {err}", task_id);
                    }
                    // 接力完成，釋放鎖
                    let mut lock = relay_lock.lock().expect("relay lock poisoned");
                    lock.remove(&task_id);
                });
            });

            let handle = app.handle().clone();
            app.listen("progress-updated", move |event| {
                let Ok(payload) = serde_json::from_str::<ProgressPayload>(event.payload()) else {
                    return;
                };
                let store = handle.state::<task_management::TaskStore>();
                let _ = store.update_progress(&payload.task_id, payload.progress);
            });

            let handle = app.handle().clone();
            app.listen("state-changed", move |event| {
                let Ok(payload) = serde_json::from_str::<StateChangedPayload>(event.payload())
                else {
                    return;
                };
                let Some(state) = task_management::TaskState::from_event_state(&payload.state)
                else {
                    return;
                };
                let store = handle.state::<task_management::TaskStore>();
                let _ = store.update_state(&payload.task_id, state);
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::create_task,
            commands::start_task,
            commands::pause_task,
            commands::resume_task,
            commands::cancel_task,
            commands::delete_task,
            commands::select_directory,
            commands::list_tasks,
            commands::extract_m3u8_options,
            commands::get_default_download_dir
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Javrate Downloader");
}
