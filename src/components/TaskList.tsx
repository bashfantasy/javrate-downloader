import { Pause, Play, Square } from "lucide-react";
import { formatFragment } from "../lib/progress";
import { statusLabels } from "../lib/taskState";
import type { DownloadTask } from "../lib/types";

interface Props {
  tasks: DownloadTask[];
  onAction: (task: DownloadTask, command: "pause_task" | "resume_task" | "cancel_task" | "delete_task") => void;
}

export function TaskList({ tasks, onAction }: Props) {
  return (
    <section className="task-section">
      <div className="task-list">
        {tasks.map((task) => (
          <article className="task-item" key={task.id}>
            <div className="task-header">
              <div className="task-title">
                <strong>{task.filename}</strong>
                <span>{task.pageUrl}</span>
                {task.m3u8Url && (
                  <span style={{ color: "#8b949e", fontSize: "0.85em", marginTop: "2px" }}>
                    {task.m3u8Url.toLowerCase().includes('.mp4') ? 'MP4: ' : 'M3U8: '}
                    {task.m3u8Url}
                  </span>
                )}
                <small>{task.saveDirectory}</small>
              </div>
              <span className={`badge badge-${task.state.toLowerCase()}`}>{statusLabels[task.state]}</span>
            </div>
            <div className={`progress-track ${task.state === "Relaying" ? "is-relaying" : ""}`}>
              <div className="progress-fill" style={{ width: `${task.progress.percentage}%` }} />
              <span className="progress-label">{task.progress.percentage.toFixed(1)}%</span>
            </div>
            {task.errorMessage && (
              <div style={{ color: "var(--danger-color)", fontSize: "0.85em", marginTop: "4px", padding: "0 4px" }}>
                錯誤: {task.errorMessage}
              </div>
            )}
            <div className="task-meta">
              <span>速度 {task.progress.speed || "-"}</span>
              <span>剩餘時間 {task.progress.eta || "-"}</span>
              <span>片段 {formatFragment(task.progress)}</span>
              {task.progress.relayAttempts > 0 && <span>接力 {task.progress.relayAttempts}/50</span>}
              {task.relayPhase && <span>{task.relayPhase}</span>}
            </div>
            <div className="task-actions">
              {task.state === "Downloading" && (
                <>
                  <button type="button" onClick={() => onAction(task, "pause_task")} title="暫停" aria-label="暫停">
                    <Pause size={16} />
                  </button>
                  <button type="button" onClick={() => onAction(task, "cancel_task")} title="取消" aria-label="取消">
                    <Square size={16} />
                  </button>
                </>
              )}
              {task.state === "Paused" && (
                <>
                  <button type="button" onClick={() => onAction(task, "resume_task")} title="恢復" aria-label="恢復">
                    <Play size={16} />
                  </button>
                  <button type="button" onClick={() => onAction(task, "cancel_task")} title="取消" aria-label="取消">
                    <Square size={16} />
                  </button>
                </>
              )}
              {(task.state === "Extracting" || task.state === "Relaying" || task.state === "Pending" || task.state === "Merging") && (
                <button type="button" onClick={() => onAction(task, "cancel_task")} title="取消" aria-label="取消">
                  <Square size={16} />
                </button>
              )}
              {(task.state === "Completed" || task.state === "Cancelled" || task.state === "Failed") && (
                <button type="button" onClick={() => onAction(task, "delete_task")} title="刪除任務" aria-label="刪除任務">
                  <span style={{ fontSize: "16px" }}>🗑️</span>
                </button>
              )}
            </div>
          </article>
        ))}
        {tasks.length === 0 && <p className="empty-state">尚無下載任務</p>}
      </div>
    </section>
  );
}
