use crate::download_engine::DownloadProgress;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum TaskState {
    Pending,
    Extracting,
    Selecting,
    Downloading,
    Merging,
    Relaying,
    Paused,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadTask {
    pub id: String,
    pub page_url: String,
    pub m3u8_url: Option<String>,
    pub selected_resolution: Option<String>,
    pub save_directory: PathBuf,
    pub filename: String,
    pub state: TaskState,
    pub progress: DownloadProgress,
    pub error_message: Option<String>,
    pub relay_phase: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateTaskInput {
    pub page_url: String,
    pub save_directory: Option<PathBuf>,
    pub filename: Option<String>,
}

#[derive(Debug, Default)]
pub struct TaskStore {
    tasks: Mutex<HashMap<String, DownloadTask>>,
    storage_path: Mutex<Option<PathBuf>>,
}

impl TaskStore {
    pub fn configure_storage(&self, path: PathBuf) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("failed to create task storage directory")?;
        }
        *self
            .storage_path
            .lock()
            .expect("storage path mutex poisoned") = Some(path);
        Ok(())
    }

    pub fn load_from_disk(&self) -> Result<()> {
        let Some(_path) = self
            .storage_path
            .lock()
            .expect("storage path mutex poisoned")
            .clone()
        else {
            return Ok(());
        };
        // NOTE: 使用者要求每次啟動時自動清空所有下載任務
        let mut tasks = self.tasks.lock().expect("task mutex poisoned");
        tasks.clear();
        drop(tasks); // 提早釋放鎖，避免 persist_current 死鎖
        self.persist_current()?;
        Ok(())
    }

    pub fn create_task(&self, input: CreateTaskInput) -> Result<DownloadTask> {
        validate_http_url(&input.page_url)?;
        let save_directory = input.save_directory.unwrap_or_else(default_download_dir);
        if !save_directory.is_dir() {
            return Err(anyhow!(
                "save directory does not exist: {}",
                save_directory.display()
            ));
        }
        let filename = input
            .filename
            .filter(|name| !name.trim().is_empty())
            .unwrap_or_else(|| derive_filename(&input.page_url));
        validate_filename(&filename)?;
        let task = DownloadTask {
            id: Uuid::new_v4().to_string(),
            page_url: input.page_url,
            m3u8_url: None,
            selected_resolution: None,
            save_directory,
            filename,
            state: TaskState::Pending,
            progress: DownloadProgress::default(),
            error_message: None,
            relay_phase: None,
        };
        self.tasks
            .lock()
            .expect("task mutex poisoned")
            .insert(task.id.clone(), task.clone());
        self.persist_current()?;
        Ok(task)
    }

    pub fn list(&self) -> Vec<DownloadTask> {
        let mut tasks: Vec<_> = self
            .tasks
            .lock()
            .expect("task mutex poisoned")
            .values()
            .cloned()
            .collect();
        tasks.sort_by(|a, b| a.id.cmp(&b.id));
        tasks
    }

    pub fn update_state(&self, task_id: &str, next: TaskState) -> Result<DownloadTask> {
        let mut tasks = self.tasks.lock().expect("task mutex poisoned");
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("task not found"))?;
        if task.state == next {
            return Ok(task.clone());
        }
        if !is_valid_transition(&task.state, &next) {
            return Err(anyhow!(
                "invalid state transition: {:?} -> {:?}",
                task.state,
                next
            ));
        }
        task.state = next;
        let updated = task.clone();
        drop(tasks);
        self.persist_current()?;
        Ok(updated)
    }

    pub fn delete_task(&self, task_id: &str) -> Result<()> {
        let mut tasks = self.tasks.lock().expect("task mutex poisoned");
        if tasks.remove(task_id).is_some() {
            drop(tasks);
            self.persist_current()?;
        }
        Ok(())
    }

    pub fn set_download_selection(
        &self,
        task_id: &str,
        m3u8_url: String,
        selected_resolution: String,
    ) -> Result<DownloadTask> {
        let mut tasks = self.tasks.lock().expect("task mutex poisoned");
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("task not found"))?;
        task.m3u8_url = Some(m3u8_url);
        task.selected_resolution = Some(selected_resolution);
        let updated = task.clone();
        drop(tasks);
        self.persist_current()?;
        Ok(updated)
    }

    pub fn update_progress(
        &self,
        task_id: &str,
        progress: DownloadProgress,
    ) -> Result<DownloadTask> {
        let mut tasks = self.tasks.lock().expect("task mutex poisoned");
        let task = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow!("task not found"))?;
        task.progress = progress;
        let updated = task.clone();
        drop(tasks);
        self.persist_current()?;
        Ok(updated)
    }

    pub fn get(&self, task_id: &str) -> Result<DownloadTask> {
        self.tasks
            .lock()
            .expect("task mutex poisoned")
            .get(task_id)
            .cloned()
            .ok_or_else(|| anyhow!("task not found"))
    }

    fn persist_current(&self) -> Result<()> {
        let Some(path) = self
            .storage_path
            .lock()
            .expect("storage path mutex poisoned")
            .clone()
        else {
            return Ok(());
        };
        let tasks = self.list();
        persist_tasks(&path, &tasks)
    }
}

impl TaskState {
    pub fn from_event_state(value: &str) -> Option<Self> {
        match value {
            "Pending" => Some(Self::Pending),
            "Extracting" => Some(Self::Extracting),
            "Selecting" => Some(Self::Selecting),
            "Downloading" => Some(Self::Downloading),
            "Merging" => Some(Self::Merging),
            "Relaying" => Some(Self::Relaying),
            "Paused" => Some(Self::Paused),
            "Completed" => Some(Self::Completed),
            "Cancelled" => Some(Self::Cancelled),
            "Failed" => Some(Self::Failed),
            _ => None,
        }
    }
}

pub fn is_valid_transition(current: &TaskState, next: &TaskState) -> bool {
    matches!(
        (current, next),
        (TaskState::Pending, TaskState::Extracting)
            | (TaskState::Pending, TaskState::Cancelled)
            | (TaskState::Pending, TaskState::Failed)
            | (TaskState::Extracting, TaskState::Selecting)
            | (TaskState::Extracting, TaskState::Downloading)
            | (TaskState::Extracting, TaskState::Cancelled)
            | (TaskState::Extracting, TaskState::Failed)
            | (TaskState::Selecting, TaskState::Downloading)
            | (TaskState::Selecting, TaskState::Cancelled)
            | (TaskState::Selecting, TaskState::Failed)
            | (TaskState::Downloading, TaskState::Completed)
            | (TaskState::Downloading, TaskState::Merging)
            | (TaskState::Downloading, TaskState::Relaying)
            | (TaskState::Downloading, TaskState::Paused)
            | (TaskState::Downloading, TaskState::Cancelled)
            | (TaskState::Downloading, TaskState::Failed)
            | (TaskState::Merging, TaskState::Completed)
            | (TaskState::Merging, TaskState::Relaying)
            | (TaskState::Merging, TaskState::Cancelled)
            | (TaskState::Merging, TaskState::Failed)
            | (TaskState::Relaying, TaskState::Downloading)
            | (TaskState::Relaying, TaskState::Cancelled)
            | (TaskState::Relaying, TaskState::Failed)
            | (TaskState::Paused, TaskState::Downloading)
            | (TaskState::Paused, TaskState::Cancelled)
            | (TaskState::Paused, TaskState::Failed)
            | (TaskState::Failed, TaskState::Relaying)
            | (TaskState::Failed, TaskState::Downloading)
            | (TaskState::Failed, TaskState::Pending)
    )
}

pub fn persist_tasks(path: &Path, tasks: &[DownloadTask]) -> Result<()> {
    let json = serde_json::to_string_pretty(tasks).context("failed to serialize task list")?;
    std::fs::write(path, json).context("failed to persist task list")
}

#[allow(dead_code)]
pub fn restore_tasks(path: &Path) -> Result<Vec<DownloadTask>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let data = std::fs::read_to_string(path).context("failed to read task list")?;
    let mut tasks: Vec<DownloadTask> =
        serde_json::from_str(&data).context("failed to parse task list")?;
    tasks.retain(|task| task.state != TaskState::Completed);
    for task in &mut tasks {
        if matches!(task.state, TaskState::Downloading | TaskState::Relaying) {
            task.state = TaskState::Paused;
        }
    }
    Ok(tasks)
}

pub fn default_download_dir() -> PathBuf {
    dirs::download_dir()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")))
}

fn validate_http_url(value: &str) -> Result<()> {
    let url = url::Url::parse(value).context("invalid URL")?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(anyhow!("URL must use http or https"));
    }
    Ok(())
}

fn validate_filename(filename: &str) -> Result<()> {
    if filename.contains('/') || filename.contains('\0') {
        return Err(anyhow!("filename must not contain path separators"));
    }
    Ok(())
}

fn derive_filename(page_url: &str) -> String {
    url::Url::parse(page_url)
        .ok()
        .and_then(|url| {
            url.path_segments()
                .and_then(|mut segments| segments.next_back().map(str::to_string))
        })
        .filter(|segment| !segment.is_empty())
        .map(|segment| segment.trim_end_matches(".html").to_string())
        .unwrap_or_else(|| "video".to_string())
        + ".mp4"
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn validates_state_matrix() {
        assert!(is_valid_transition(
            &TaskState::Downloading,
            &TaskState::Paused
        ));
        assert!(is_valid_transition(
            &TaskState::Merging,
            &TaskState::Relaying
        ));
        assert!(!is_valid_transition(
            &TaskState::Completed,
            &TaskState::Paused
        ));
    }

    #[test]
    fn restores_active_downloads_as_paused() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("tasks.json");
        let task = DownloadTask {
            id: "a".into(),
            page_url: "https://example.com/watch/1".into(),
            m3u8_url: None,
            selected_resolution: None,
            save_directory: dir.path().to_path_buf(),
            filename: "one.mp4".into(),
            state: TaskState::Downloading,
            progress: DownloadProgress::default(),
            error_message: None,
            relay_phase: None,
        };
        persist_tasks(&path, &[task]).unwrap();
        let restored = restore_tasks(&path).unwrap();
        assert_eq!(restored[0].state, TaskState::Paused);
    }

    #[test]
    fn creates_task_with_custom_save_path_and_filename() {
        let dir = tempdir().unwrap();
        let store = TaskStore::default();
        let task = store
            .create_task(CreateTaskInput {
                page_url: "https://example.com/watch/one".into(),
                save_directory: Some(dir.path().to_path_buf()),
                filename: Some("custom-name.mp4".into()),
            })
            .unwrap();

        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.save_directory, dir.path());
        assert_eq!(task.filename, "custom-name.mp4");
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn rejects_invalid_url_and_missing_directory() {
        let store = TaskStore::default();
        assert!(store
            .create_task(CreateTaskInput {
                page_url: "not a url".into(),
                save_directory: None,
                filename: None,
            })
            .is_err());

        assert!(store
            .create_task(CreateTaskInput {
                page_url: "https://example.com/watch/one".into(),
                save_directory: Some(PathBuf::from("/path/that/does/not/exist")),
                filename: Some("one.mp4".into()),
            })
            .is_err());
    }

    #[test]
    fn multiple_tasks_update_independently() {
        let dir = tempdir().unwrap();
        let store = TaskStore::default();
        let first = store
            .create_task(CreateTaskInput {
                page_url: "https://example.com/watch/one".into(),
                save_directory: Some(dir.path().to_path_buf()),
                filename: Some("one.mp4".into()),
            })
            .unwrap();
        let second = store
            .create_task(CreateTaskInput {
                page_url: "https://example.com/watch/two".into(),
                save_directory: Some(dir.path().to_path_buf()),
                filename: Some("two.mp4".into()),
            })
            .unwrap();

        store
            .update_state(&first.id, TaskState::Extracting)
            .unwrap();
        assert_eq!(store.get(&first.id).unwrap().state, TaskState::Extracting);
        assert_eq!(store.get(&second.id).unwrap().state, TaskState::Pending);
    }
}
