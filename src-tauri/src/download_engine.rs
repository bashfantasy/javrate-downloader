use crate::extraction::{origin_from_url, SAFARI_USER_AGENT};
use anyhow::{anyhow, Context, Result};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{ExitStatus, Stdio};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::oneshot;
use tokio::time::{timeout, Duration};

#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub percentage: f64,
    pub speed: String,
    pub eta: String,
    pub current_fragment: Option<u32>,
    pub total_fragments: Option<u32>,
    pub relay_attempts: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadConfig {
    pub task_id: String,
    pub page_url: String,
    pub m3u8_url: String,
    pub save_directory: PathBuf,
    pub filename: String,
    pub thread_count: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgressPayload {
    pub task_id: String,
    pub progress: DownloadProgress,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RelayNeededPayload {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompletionPayload {
    pub task_id: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessExitedPayload {
    pub task_id: String,
    pub success: bool,
    pub code: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StateChangedPayload {
    pub task_id: String,
    pub state: String,
    pub error_message: Option<String>,
    pub relay_phase: Option<String>,
}

#[derive(Debug)]
pub struct RunningDownload {
    pub pid: u32,
    pub config: DownloadConfig,
    exit_rx: oneshot::Receiver<ExitStatus>,
}

#[derive(Debug)]
struct RunningProcess {
    pid: u32,
    config: DownloadConfig,
    exit_rx: Option<oneshot::Receiver<ExitStatus>>,
}

#[derive(Debug, Default)]
pub struct DownloadProcessRegistry {
    processes: Mutex<HashMap<String, RunningProcess>>,
}

impl DownloadProcessRegistry {
    pub fn insert(&self, running: RunningDownload) {
        self.processes
            .lock()
            .expect("process registry mutex poisoned")
            .insert(
                running.config.task_id.clone(),
                RunningProcess {
                    pid: running.pid,
                    config: running.config,
                    exit_rx: Some(running.exit_rx),
                },
            );
    }

    pub fn get_config(&self, task_id: &str) -> Result<DownloadConfig> {
        self.processes
            .lock()
            .expect("process registry mutex poisoned")
            .get(task_id)
            .map(|process| process.config.clone())
            .ok_or_else(|| anyhow!("download process not found"))
    }

    pub async fn signal_and_wait(
        &self,
        task_id: &str,
        signal: nix::sys::signal::Signal,
    ) -> Result<()> {
        let (pid, exit_rx) = {
            let mut processes = self
                .processes
                .lock()
                .expect("process registry mutex poisoned");
            let process = processes
                .get_mut(task_id)
                .ok_or_else(|| anyhow!("download process not found"))?;
            (process.pid, process.exit_rx.take())
        };

        send_signal_to_pid(pid, signal)?;

        if let Some(exit_rx) = exit_rx {
            let _ = timeout(Duration::from_secs(30), exit_rx)
                .await
                .context("timed out waiting for yt-dlp to exit")?;
        }

        self.processes
            .lock()
            .expect("process registry mutex poisoned")
            .remove(task_id);
        Ok(())
    }
}

pub fn build_yt_dlp_args(config: &DownloadConfig) -> Result<Vec<String>> {
    let output_path = output_path(&config.save_directory, &config.filename)?;
    let origin = origin_from_url(&config.page_url)?;
    // NOTE: 使用 origin（scheme + host）作為 Referer，而非完整 page_url。
    // 完整 URL 可能含有中文等非 ASCII 字元，導致 yt-dlp 內部 HTTP 函式庫
    // 在以 latin-1 編碼 header 時拋出 UnicodeEncodeError。
    Ok(vec![
        "--newline".to_string(),
        "--merge-output-format".to_string(),
        "mp4".to_string(),
        "-N".to_string(),
        config.thread_count.max(1).to_string(),
        "-o".to_string(),
        output_path.to_string_lossy().to_string(),
        "--add-header".to_string(),
        format!("Referer: {}", origin),
        "--add-header".to_string(),
        format!("Origin: {}", origin),
        "--add-header".to_string(),
        format!("User-Agent: {}", SAFARI_USER_AGENT),
        config.m3u8_url.clone(),
    ])
}

pub async fn ensure_yt_dlp_available() -> Result<()> {
    let status = Command::new("yt-dlp")
        .arg("--version")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .context("yt-dlp is not installed or not available on PATH")?;
    if !status.success() {
        return Err(anyhow!("yt-dlp is not installed or not available on PATH"));
    }
    Ok(())
}

pub async fn spawn_download(config: DownloadConfig, app: AppHandle) -> Result<RunningDownload> {
    ensure_yt_dlp_available().await?;
    let args = build_yt_dlp_args(&config)?;
    let mut child = Command::new("yt-dlp")
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to spawn yt-dlp")?;
    let pid = child
        .id()
        .ok_or_else(|| anyhow!("yt-dlp process has no pid"))?;
    let latest_progress = Arc::new(Mutex::new(DownloadProgress::default()));
    // NOTE: 共享的「需要接力」旗標，由 monitor_output 設定、monitor_exit 讀取。
    // 確保接力事件只在 yt-dlp 進程自然退出後才觸發，避免 fragment 檔案衝突。
    let needs_relay = Arc::new(std::sync::atomic::AtomicBool::new(false));

    if let Some(stdout) = child.stdout.take() {
        monitor_output(
            config.task_id.clone(),
            stdout,
            app.clone(),
            Arc::clone(&latest_progress),
            Arc::clone(&needs_relay),
        );
    }
    if let Some(stderr) = child.stderr.take() {
        monitor_output(
            config.task_id.clone(),
            stderr,
            app.clone(),
            Arc::clone(&latest_progress),
            Arc::clone(&needs_relay),
        );
    }
    let (exit_tx, exit_rx) = oneshot::channel();
    monitor_exit(config.task_id.clone(), child, app, latest_progress, exit_tx, needs_relay);

    Ok(RunningDownload {
        pid,
        config,
        exit_rx,
    })
}

pub fn parse_progress_line(line: &str) -> Option<DownloadProgress> {
    let re = Regex::new(
        r"\[download\]\s+(?P<pct>\d+(?:\.\d+)?)%.*?at\s+(?P<speed>\S+)\s+ETA\s+(?P<eta>\S+)(?:\s+\(?frag\s+(?P<cur>\d+)/(?P<total>\d+)\)?)?",
    )
    .expect("valid regex");
    let captures = re.captures(line)?;
    Some(DownloadProgress {
        percentage: captures.name("pct")?.as_str().parse().ok()?,
        speed: captures
            .name("speed")
            .map(|m| m.as_str())
            .unwrap_or("")
            .to_string(),
        eta: captures
            .name("eta")
            .map(|m| m.as_str())
            .unwrap_or("")
            .to_string(),
        current_fragment: captures.name("cur").and_then(|m| m.as_str().parse().ok()),
        total_fragments: captures.name("total").and_then(|m| m.as_str().parse().ok()),
        relay_attempts: 0,
    })
}

pub fn is_403_line(line: &str) -> bool {
    line.contains("HTTP Error 403")
        || line.contains("403 Forbidden")
        || line.contains("HTTP error 403")
}

pub fn is_merging_line(line: &str) -> bool {
    line.contains("[Merger]") 
        || line.contains("[Fixup") 
        || (line.contains("[ffmpeg]") && (line.contains("Merging") || line.contains("Fixing")))
        || line.contains("Merging formats into")
}

pub fn send_signal_to_pid(pid: u32, signal: nix::sys::signal::Signal) -> Result<()> {
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), signal)
        .context("failed to signal yt-dlp")
}

fn monitor_output<R>(
    task_id: String,
    stream: R,
    app: AppHandle,
    latest_progress: Arc<Mutex<DownloadProgress>>,
    needs_relay: Arc<std::sync::atomic::AtomicBool>,
) where
    R: tokio::io::AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(stream).lines();

        while let Ok(Some(line)) = lines.next_line().await {
            // 加入日誌輸出以便除錯
            use std::io::Write;
            if let Ok(mut file) = std::fs::OpenOptions::new().create(true).append(true).open("/tmp/javrate-ytdlp.log") {
                let _ = writeln!(file, "[{}] {}", task_id, line);
            }

            if let Some(progress) = parse_progress_line(&line) {
                *latest_progress.lock().expect("progress mutex poisoned") = progress.clone();
                let _ = app.emit(
                    "progress-updated",
                    ProgressPayload {
                        task_id: task_id.clone(),
                        progress,
                    },
                );
            }
            if is_403_line(&line) {
                // NOTE: 只設旗標，不立即觸發接力。
                // 等 yt-dlp 自然退出後由 monitor_exit 觸發，避免兩個進程同時存取 .part 檔案。
                needs_relay.store(true, std::sync::atomic::Ordering::SeqCst);
            } else if is_merging_line(&line) {
                let _ = app.emit(
                    "state-changed",
                    StateChangedPayload {
                        task_id: task_id.clone(),
                        state: "Merging".to_string(),
                        error_message: None,
                        relay_phase: None,
                    },
                );
            }
        }
    });
}

fn monitor_exit(
    task_id: String,
    mut child: Child,
    app: AppHandle,
    latest_progress: Arc<Mutex<DownloadProgress>>,
    exit_tx: oneshot::Sender<ExitStatus>,
    needs_relay: Arc<std::sync::atomic::AtomicBool>,
) {
    tokio::spawn(async move {
        if let Ok(status) = child.wait().await {
            let progress = latest_progress
                .lock()
                .expect("progress mutex poisoned")
                .clone();
            
            // 如果遇到 403 錯誤，我們必須無視 yt-dlp 的退出碼，強制進行接力。
            // 這是因為如果 yt-dlp 內部因 Python 例外崩潰，或者因放棄重試而退出，
            // 它有時會錯誤地返回 exit code 0，導致我們誤判為下載完成。
            let requires_relay = needs_relay.load(std::sync::atomic::Ordering::SeqCst);
            let completed = !requires_relay && is_download_complete(status.success(), &progress);

            if completed {
                let _ = app.emit(
                    "download-completed",
                    CompletionPayload {
                        task_id: task_id.clone(),
                    },
                );
                let _ = app.emit(
                    "state-changed",
                    StateChangedPayload {
                        task_id: task_id.clone(),
                        state: "Completed".to_string(),
                        error_message: None,
                        relay_phase: None,
                    },
                );
            }

            let _ = app.emit(
                "download-process-exited",
                ProcessExitedPayload {
                    task_id: task_id.clone(),
                    success: completed,
                    code: status.code(),
                },
            );
            let _ = exit_tx.send(status);

            // NOTE: 進程已完全退出，.part 檔案處於穩定狀態。
            // 此時才觸發接力，新 yt-dlp 可安全續傳，零 fragment 衝突。
            if requires_relay {
                let _ = app.emit(
                    "relay-needed",
                    RelayNeededPayload {
                        task_id,
                    },
                );
            }
        }
    });
}

pub fn is_download_complete(status_success: bool, _progress: &DownloadProgress) -> bool {
    status_success
}

fn output_path(save_directory: &Path, filename: &str) -> Result<PathBuf> {
    if filename.trim().is_empty() || filename.contains('/') {
        return Err(anyhow!("filename must be a non-empty file name"));
    }
    Ok(save_directory.join(filename))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_standard_progress_lines() {
        let progress = parse_progress_line(
            "[download]  45.3% of ~128.50MiB at 5.20MiB/s ETA 00:15 frag 92/203",
        )
        .unwrap();
        assert_eq!(progress.percentage, 45.3);
        assert_eq!(progress.speed, "5.20MiB/s");
        assert_eq!(progress.eta, "00:15");
        assert_eq!(progress.current_fragment, Some(92));
        assert_eq!(progress.total_fragments, Some(203));

        let progress2 = parse_progress_line(
            "[download]  17.3% of ~   3.26GiB at   10.86MiB/s ETA 04:12 (frag 310/1789)",
        )
        .unwrap();
        assert_eq!(progress2.percentage, 17.3);
        assert_eq!(progress2.speed, "10.86MiB/s");
        assert_eq!(progress2.eta, "04:12");
        assert_eq!(progress2.current_fragment, Some(310));
        assert_eq!(progress2.total_fragments, Some(1789));
    }

    #[test]
    fn ignores_non_progress_lines() {
        assert!(parse_progress_line("[info] downloading webpage").is_none());
    }

    #[test]
    fn detects_403_variants() {
        assert!(is_403_line("HTTP Error 403: Forbidden"));
        assert!(is_403_line("403 Forbidden"));
        assert!(is_403_line("HTTP error 403"));
    }

    #[test]
    fn builds_yt_dlp_args_with_headers_and_output_path() {
        let config = DownloadConfig {
            task_id: "task-1".into(),
            page_url: "https://example.com/watch/one".into(),
            m3u8_url: "https://cdn.example.com/video/720p/index.m3u8?token=abc".into(),
            save_directory: PathBuf::from("/tmp/downloads"),
            filename: "one.mp4".into(),
            thread_count: 20,
        };

        let args = build_yt_dlp_args(&config).unwrap();
        assert_eq!(args[0], "--newline");
        assert_eq!(args[1], "--merge-output-format");
        assert_eq!(args[2], "mp4");
        assert_eq!(args[3], "-N");
        assert_eq!(args[4], "20");
        assert!(args.contains(&"/tmp/downloads/one.mp4".to_string()));
        assert!(args.contains(&"Referer: https://example.com".to_string()));
        assert!(args.contains(&"Origin: https://example.com".to_string()));
        assert!(args.contains(&config.m3u8_url));
    }

    #[test]
    fn detects_completion_only_after_successful_exit() {
        assert!(is_download_complete(
            true,
            &DownloadProgress {
                percentage: 50.0,
                ..DownloadProgress::default()
            }
        ));
        assert!(!is_download_complete(
            false,
            &DownloadProgress {
                percentage: 100.0,
                ..DownloadProgress::default()
            }
        ));
    }

    #[test]
    fn registry_keeps_multiple_task_configs_independently() {
        let registry = DownloadProcessRegistry::default();
        let (first_tx, first_rx) = tokio::sync::oneshot::channel();
        let (second_tx, second_rx) = tokio::sync::oneshot::channel();
        drop(first_tx);
        drop(second_tx);

        registry.insert(RunningDownload {
            pid: u32::MAX - 1,
            config: DownloadConfig {
                task_id: "task-a".into(),
                page_url: "https://example.com/a".into(),
                m3u8_url: "https://cdn.example.com/a.m3u8".into(),
                save_directory: PathBuf::from("/tmp/a"),
                filename: "a.mp4".into(),
                thread_count: 20,
            },
            exit_rx: first_rx,
        });
        registry.insert(RunningDownload {
            pid: u32::MAX,
            config: DownloadConfig {
                task_id: "task-b".into(),
                page_url: "https://example.com/b".into(),
                m3u8_url: "https://cdn.example.com/b.m3u8".into(),
                save_directory: PathBuf::from("/tmp/b"),
                filename: "b.mp4".into(),
                thread_count: 20,
            },
            exit_rx: second_rx,
        });

        assert_eq!(registry.get_config("task-a").unwrap().filename, "a.mp4");
        assert_eq!(registry.get_config("task-b").unwrap().filename, "b.mp4");
    }
}
