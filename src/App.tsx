import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { ClipboardPaste, FolderOpen } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { ResolutionDialog } from "./components/ResolutionDialog";
import { TaskList } from "./components/TaskList";
import { createEmptyProgress, mergeProgress } from "./lib/progress";
import { deriveFilename, isValidHttpUrl } from "./lib/m3u8";
import type { DownloadTask, M3u8Option, ProgressEventPayload, StateEventPayload } from "./lib/types";



export function App() {
  const [url, setUrl] = useState("");
  const [saveDirectory, setSaveDirectory] = useState("");
  const [filename, setFilename] = useState("video.mp4");
  const [tasks, setTasks] = useState<DownloadTask[]>([]);
  const [error, setError] = useState("");
  const [resolutionTask, setResolutionTask] = useState<DownloadTask | null>(null);
  const [resolutionOptions, setResolutionOptions] = useState<M3u8Option[]>([]);

  useEffect(() => {
    // 從後端取得系統預設下載目錄，避免硬編碼路徑
    void invoke<string>("get_default_download_dir").then(setSaveDirectory).catch(() => {});
    void invoke<DownloadTask[]>("list_tasks").then(setTasks).catch(() => setTasks([]));
    const unlistenProgress = listen<ProgressEventPayload>("progress-updated", (event) => {
      setTasks((current) =>
        current.map((task) =>
          task.id === event.payload.taskId
            ? { ...task, progress: mergeProgress(task.progress, event.payload.progress) }
            : task,
        ),
      );
    });
    const unlistenState = listen<StateEventPayload>("state-changed", (event) => {
      setTasks((current) =>
        current.map((task) =>
          task.id === event.payload.taskId
            ? {
                ...task,
                state: event.payload.state,
                errorMessage: event.payload.errorMessage,
                relayPhase: event.payload.relayPhase,
                ...(event.payload.state === "Completed" && {
                  progress: { ...task.progress, percentage: 100 },
                }),
              }
            : task,
        ),
      );
    });
    return () => {
      void unlistenProgress.then((fn) => fn());
      void unlistenState.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    if (url.trim()) {
      setFilename(deriveFilename(url.trim()));
    }
  }, [url]);

  const canSubmit = useMemo(() => url.trim() && saveDirectory.trim() && filename.trim(), [url, saveDirectory, filename]);

  async function chooseDirectory() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected === "string") {
      setSaveDirectory(selected);
    }
  }

  async function submit() {
    setError("");
    if (!isValidHttpUrl(url)) {
      setError("URL 格式無效");
      return;
    }
    if (!saveDirectory.trim()) {
      setError("存檔目錄不存在");
      return;
    }

    try {
      let finalFilename = filename.trim();
      if (!finalFilename.toLowerCase().endsWith(".mp4")) {
        finalFilename += ".mp4";
      }

      const task = await invoke<DownloadTask>("create_task", {
        request: { pageUrl: url.trim(), saveDirectory: saveDirectory.trim(), filename: finalFilename },
      });
      const normalized = { ...task, progress: task.progress ?? createEmptyProgress() };
      setTasks((current) => [normalized, ...current]);
      setUrl("");
      setFilename("video.mp4");

      const extraction = await invoke<{ options: M3u8Option[] }>("extract_m3u8_options", { pageUrl: task.pageUrl });
      if (extraction.options.length > 1) {
        setResolutionTask(normalized);
        setResolutionOptions(extraction.options);
        return;
      }
      if (extraction.options[0]) {
        await startDownload(normalized, extraction.options[0]);
      }
    } catch (err) {
      setError(String(err));
    }
  }

  async function startDownload(task: DownloadTask, option: M3u8Option) {
    try {
      const updated = await invoke<DownloadTask>("start_task", {
        taskId: task.id,
        m3u8Url: option.url,
        resolution: option.resolution,
      });
      setTasks((current) => current.map((item) => (item.id === task.id ? updated : item)));
    } catch (err) {
      setError(String(err));
    } finally {
      setResolutionTask(null);
      setResolutionOptions([]);
    }
  }

  async function taskAction(task: DownloadTask, command: "pause_task" | "resume_task" | "cancel_task" | "delete_task") {
    if (command === "delete_task") {
      await invoke("delete_task", { taskId: task.id });
      setTasks((current) => current.filter((item) => item.id !== task.id));
      return;
    }
    const updated = await invoke<DownloadTask>(command, { taskId: task.id });
    setTasks((current) => current.map((item) => (item.id === task.id ? updated : item)));
  }

  return (
    <main className="app-shell">
      <section className="top-panel">
        <div className="brand-row">
          <h1>Javrate Downloader</h1>
          <span>m3u8 relay engine</span>
        </div>
        <div className="input-grid">
          <label className="field field-url">
            <span>影片頁面 URL</span>
            <div className="paste-row">
              <input value={url} onChange={(event) => setUrl(event.target.value)} placeholder="https://example.com/watch/..." />
              <button className="icon-button" type="button" onClick={() => navigator.clipboard.readText().then(setUrl)} aria-label="貼上網址" title="貼上網址">
                <ClipboardPaste size={16} />
              </button>
            </div>
          </label>
          <label className="field field-path">
            <span>存檔目錄</span>
            <div className="path-row">
              <input value={saveDirectory} onChange={(event) => setSaveDirectory(event.target.value)} />
              <button className="icon-button" type="button" onClick={chooseDirectory} aria-label="選擇資料夾" title="選擇資料夾">
                <FolderOpen size={18} />
              </button>
            </div>
          </label>
          <label className="field field-name">
            <span>檔名</span>
            <div className="paste-row">
              <input value={filename} onChange={(event) => setFilename(event.target.value)} />
              <button className="icon-button" type="button" onClick={() => navigator.clipboard.readText().then(setFilename)} aria-label="貼上檔名" title="貼上檔名">
                <ClipboardPaste size={16} />
              </button>
            </div>
          </label>
          <button className="primary-button" type="button" disabled={!canSubmit} onClick={() => void submit()}>
            開始下載
          </button>
        </div>
        {error && <p className="inline-error">{error}</p>}
      </section>

      <TaskList tasks={tasks} onAction={(task, command) => void taskAction(task, command)} />

      {resolutionTask && (
        <ResolutionDialog
          options={resolutionOptions}
          onCancel={() => setResolutionTask(null)}
          onConfirm={(option) => void startDownload(resolutionTask, option)}
        />
      )}
    </main>
  );
}
