use crate::download_engine::{
    spawn_download, DownloadConfig, DownloadProcessRegistry, StateChangedPayload,
};
use crate::extraction::{choose_resolution, extract_from_page};
use crate::task_management::{TaskState, TaskStore};
use anyhow::{anyhow, Result};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
pub const MAX_RELAY_ATTEMPTS: u32 = 30;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayStatusPayload {
    pub task_id: String,
    pub attempt: u32,
    pub phase: String,
}

pub async fn handle_relay_needed(
    store: &TaskStore,
    processes: &DownloadProcessRegistry,
    app: AppHandle,
    task_id: &str,
) -> Result<()> {
    match handle_relay_inner(store, processes, app.clone(), task_id).await {
        Ok(_) => Ok(()),
        Err(e) => {
            let _ = store.update_state(task_id, TaskState::Failed);
            emit_state(&app, task_id, "Failed", Some(&e.to_string()));
            Err(e)
        }
    }
}

async fn handle_relay_inner(
    store: &TaskStore,
    processes: &DownloadProcessRegistry,
    app: AppHandle,
    task_id: &str,
) -> Result<()> {
    // 延遲 3 秒，防止遇到致命錯誤時無窮快速重試導致程式崩潰
    tokio::time::sleep(tokio::time::Duration::from_secs(3)).await;

    let mut task = store.get(task_id)?;

    if !relay_attempt_allowed(task.progress.relay_attempts + 1) {
        store.update_state(&task_id, TaskState::Failed)?;
        emit_state(&app, &task_id, "Failed", Some("relay retry limit exceeded"));
        return Err(anyhow!("relay retry limit exceeded"));
    }

    store.update_state(task_id, TaskState::Relaying)?;
    task.progress.relay_attempts += 1;
    store.update_progress(task_id, task.progress.clone())?;
    emit_state(&app, task_id, "Relaying", None);
    emit_phase(
        &app,
        task_id,
        task.progress.relay_attempts,
        "Re-extracting m3u8 URL",
    );

    // 加上隨機參數避免讀取到舊的快取 HTML
    let mut cache_busted_url = task.page_url.clone();
    if cache_busted_url.contains('?') {
        cache_busted_url.push_str(&format!("&_t={}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    } else {
        cache_busted_url.push_str(&format!("?_t={}", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis()));
    }

    let extraction = extract_from_page(&app, &cache_busted_url).await?;
    let original_resolution = task.selected_resolution.as_deref();
    let selected = task
        .selected_resolution
        .as_deref()
        .and_then(|resolution| choose_resolution(&extraction.options, resolution))
        .or_else(|| extraction.options.first().cloned())
        .ok_or_else(|| anyhow!("no refreshed m3u8 URL available"))?;
    let mut final_url = selected.url.clone();
    if let Some(original_resolution) = original_resolution {
        if selected.resolution != original_resolution {
            eprintln!(
                "resolution mismatch during relay for task {}: wanted {}, selected {}",
                task_id, original_resolution, selected.resolution
            );
            // 嘗試將新的 token 補丁到舊的 URL 上
            if let Some(old_m3u8) = &task.m3u8_url {
                final_url = crate::cdn_adapter::patch_m3u8_url(&selected.url, old_m3u8);
            }
        }
    }

    // 記錄準備使用的接力網址
    use std::io::Write;
    if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/javrate-ytdlp.log") {
        let _ = writeln!(file, "[{}] RELAY ATTEMPT {} USING URL: {}", task_id, task.progress.relay_attempts, final_url);
    }

    emit_phase(
        &app,
        task_id,
        task.progress.relay_attempts,
        "Restarting download",
    );
    restart_download(app.clone(), processes, &task, final_url).await?;
    store.update_state(task_id, TaskState::Downloading)?;
    emit_state(&app, task_id, "Downloading", None);
    Ok(())
}

async fn restart_download(
    app: AppHandle,
    processes: &DownloadProcessRegistry,
    task: &crate::task_management::DownloadTask,
    final_url: String,
) -> Result<()> {
    // NOTE: 舊 yt-dlp 已在 monitor_exit 中自然退出後才觸發接力，
    // 不需要 SIGTERM，.part 檔案處於穩定狀態，可安全續傳。
    let config = DownloadConfig {
        task_id: task.id.clone(),
        page_url: task.page_url.clone(),
        m3u8_url: final_url,
        save_directory: task.save_directory.clone(),
        filename: task.filename.clone(),
        thread_count: 20,
    };
    let running = spawn_download(config, app).await?;
    processes.insert(running);
    Ok(())
}

fn emit_phase(app: &AppHandle, task_id: &str, attempt: u32, phase: &str) {
    let _ = app.emit(
        "relay-status",
        RelayStatusPayload {
            task_id: task_id.to_string(),
            attempt,
            phase: format!(
                "Relay attempt {}/{}: {}...",
                attempt, MAX_RELAY_ATTEMPTS, phase
            ),
        },
    );
}

fn emit_state(app: &AppHandle, task_id: &str, state: &str, error_message: Option<&str>) {
    let _ = app.emit(
        "state-changed",
        StateChangedPayload {
            task_id: task_id.to_string(),
            state: state.to_string(),
            error_message: error_message.map(str::to_string),
            relay_phase: None,
        },
    );
}

pub fn relay_attempt_allowed(next_attempt: u32) -> bool {
    next_attempt <= MAX_RELAY_ATTEMPTS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allows_attempt_fifty_as_last_attempt() {
        assert_eq!(MAX_RELAY_ATTEMPTS, 30);
        assert!(relay_attempt_allowed(1));
        assert!(relay_attempt_allowed(25));
        assert!(relay_attempt_allowed(30));
        assert!(!relay_attempt_allowed(31));
    }

    #[test]
    fn test_patch_m3u8_url() {
        let old_url = "https://videocdn.avking.xyz/bcdn_token=OLD_TOKEN&expires=1000&token_path=%2Fabc%2F/abc/720p/video.m3u8";
        let new_url = "https://videocdn.avking.xyz/bcdn_token=NEW_TOKEN&expires=2000&token_path=%2Fabc%2F/abc/playlist.m3u8";
        let patched = crate::cdn_adapter::patch_m3u8_url(new_url, old_url);
        assert_eq!(patched, "https://videocdn.avking.xyz/bcdn_token=NEW_TOKEN&expires=2000&token_path=%2Fabc%2F/abc/720p/video.m3u8");
    }
}
