use crate::download_engine::{spawn_download, DownloadConfig, DownloadProcessRegistry};
use crate::extraction::{extract_from_page, ExtractionResult};
use crate::task_management::{CreateTaskInput, DownloadTask, TaskState, TaskStore};
use anyhow::Result;
use serde::Deserialize;
use std::path::PathBuf;
use tauri::{AppHandle, State};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskRequest {
    pub page_url: String,
    pub save_directory: Option<PathBuf>,
    pub filename: Option<String>,
}

#[tauri::command]
pub fn create_task(
    store: State<'_, TaskStore>,
    request: CreateTaskRequest,
) -> Result<DownloadTask, String> {
    store
        .create_task(CreateTaskInput {
            page_url: request.page_url,
            save_directory: request.save_directory,
            filename: request.filename,
        })
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub fn list_tasks(store: State<'_, TaskStore>) -> Vec<DownloadTask> {
    store.list()
}

#[tauri::command]
pub async fn extract_m3u8_options(
    app: AppHandle,
    page_url: String,
) -> Result<ExtractionResult, String> {
    extract_from_page(&app, &page_url)
        .await
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn start_task(
    app: AppHandle,
    store: State<'_, TaskStore>,
    processes: State<'_, DownloadProcessRegistry>,
    task_id: String,
    m3u8_url: String,
    resolution: String,
) -> Result<DownloadTask, String> {
    let task = store.get(&task_id).map_err(|err| err.to_string())?;
    if task.state == TaskState::Pending {
        let _ = store.update_state(&task_id, TaskState::Extracting);
    }
    let config = DownloadConfig {
        task_id: task.id.clone(),
        page_url: task.page_url.clone(),
        m3u8_url: m3u8_url.clone(),
        save_directory: task.save_directory.clone(),
        filename: task.filename.clone(),
        thread_count: 20,
    };
    match spawn_download(config, app).await {
        Ok(running) => processes.insert(running),
        Err(err) => {
            let _ = store.update_state(&task_id, TaskState::Failed);
            return Err(err.to_string());
        }
    }
    let _ = store.set_download_selection(&task_id, m3u8_url, resolution);
    let updated = store
        .update_state(&task_id, TaskState::Downloading)
        .map_err(|err| err.to_string())?;
    Ok(updated)
}

#[tauri::command]
pub async fn pause_task(
    store: State<'_, TaskStore>,
    processes: State<'_, DownloadProcessRegistry>,
    task_id: String,
) -> Result<DownloadTask, String> {
    processes
        .signal_and_wait(&task_id, nix::sys::signal::Signal::SIGINT)
        .await
        .map_err(|err| err.to_string())?;
    store
        .update_state(&task_id, TaskState::Paused)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn resume_task(
    app: AppHandle,
    store: State<'_, TaskStore>,
    processes: State<'_, DownloadProcessRegistry>,
    task_id: String,
) -> Result<DownloadTask, String> {
    let config = processes
        .get_config(&task_id)
        .or_else(|_| {
            let task = store.get(&task_id)?;
            Ok::<DownloadConfig, anyhow::Error>(DownloadConfig {
                task_id: task.id.clone(),
                page_url: task.page_url,
                m3u8_url: task
                    .m3u8_url
                    .ok_or_else(|| anyhow::anyhow!("task has no selected m3u8 URL"))?,
                save_directory: task.save_directory,
                filename: task.filename,
                thread_count: 20,
            })
        })
        .map_err(|err| err.to_string())?;
    let running = spawn_download(config, app)
        .await
        .map_err(|err| err.to_string())?;
    processes.insert(running);
    store
        .update_state(&task_id, TaskState::Downloading)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn cancel_task(
    store: State<'_, TaskStore>,
    processes: State<'_, DownloadProcessRegistry>,
    task_id: String,
) -> Result<DownloadTask, String> {
    if store
        .get(&task_id)
        .map(|task| matches!(task.state, TaskState::Downloading | TaskState::Relaying))
        .unwrap_or(false)
    {
        processes
            .signal_and_wait(&task_id, nix::sys::signal::Signal::SIGTERM)
            .await
            .map_err(|err| err.to_string())?;
    }
    store
        .update_state(&task_id, TaskState::Cancelled)
        .map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn delete_task(task_id: String, store: State<'_, TaskStore>) -> Result<(), String> {
    store.delete_task(&task_id).map_err(|err| err.to_string())
}

#[tauri::command]
pub async fn select_directory() -> Result<Option<PathBuf>, String> {
    Ok(None)
}

/// 回傳系統預設的下載目錄路徑
#[tauri::command]
pub fn get_default_download_dir() -> String {
    crate::task_management::default_download_dir()
        .to_string_lossy()
        .to_string()
}
