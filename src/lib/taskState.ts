import type { TaskState } from "./types";

type Action = "pause" | "resume" | "cancel" | "relay" | "complete" | "fail" | "extract" | "select" | "download" | "merge";

const transitions: Record<TaskState, Partial<Record<Action, TaskState>>> = {
  Pending: { extract: "Extracting", cancel: "Cancelled", fail: "Failed" },
  Extracting: { select: "Selecting", download: "Downloading", cancel: "Cancelled", fail: "Failed" },
  Selecting: { download: "Downloading", cancel: "Cancelled", fail: "Failed" },
  Downloading: { pause: "Paused", cancel: "Cancelled", relay: "Relaying", merge: "Merging", complete: "Completed", fail: "Failed" },
  Merging: { complete: "Completed", relay: "Relaying", cancel: "Cancelled", fail: "Failed" },
  Relaying: { download: "Downloading", cancel: "Cancelled", fail: "Failed" },
  Paused: { resume: "Downloading", cancel: "Cancelled", fail: "Failed" },
  Completed: {},
  Cancelled: {},
  Failed: {},
};

export function nextTaskState(current: TaskState, action: Action): TaskState | null {
  return transitions[current][action] ?? null;
}

export function assertValidTransition(current: TaskState, action: Action): TaskState {
  const next = nextTaskState(current, action);
  if (!next) {
    throw new Error(`Invalid task transition: ${current} -> ${action}`);
  }
  return next;
}

export const statusLabels: Record<TaskState, string> = {
  Pending: "等待中",
  Extracting: "解析中",
  Selecting: "選擇解析度",
  Downloading: "下載中",
  Merging: "合併中",
  Relaying: "接力中",
  Paused: "已暫停",
  Completed: "已完成",
  Cancelled: "已取消",
  Failed: "失敗",
};
