export type TaskState =
  | "Pending"
  | "Extracting"
  | "Selecting"
  | "Downloading"
  | "Merging"
  | "Relaying"
  | "Paused"
  | "Completed"
  | "Cancelled"
  | "Failed";

export interface DownloadProgress {
  percentage: number;
  speed: string;
  eta: string;
  currentFragment: number | null;
  totalFragments: number | null;
  relayAttempts: number;
}

export interface M3u8Option {
  url: string;
  resolution: string;
}

export interface DownloadTask {
  id: string;
  pageUrl: string;
  m3u8Url?: string;
  selectedResolution?: string;
  saveDirectory: string;
  filename: string;
  state: TaskState;
  progress: DownloadProgress;
  errorMessage?: string;
  relayPhase?: string;
}

export interface CreateTaskInput {
  pageUrl: string;
  saveDirectory: string;
  filename: string;
}

export interface ProgressEventPayload {
  taskId: string;
  progress: Partial<DownloadProgress>;
}

export interface StateEventPayload {
  taskId: string;
  state: TaskState;
  errorMessage?: string;
  relayPhase?: string;
}
