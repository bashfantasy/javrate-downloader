import type { DownloadProgress } from "./types";

export function createEmptyProgress(): DownloadProgress {
  return {
    percentage: 0,
    speed: "",
    eta: "",
    currentFragment: null,
    totalFragments: null,
    relayAttempts: 0,
  };
}

export function mergeProgress(current: DownloadProgress, update: Partial<DownloadProgress>): DownloadProgress {
  const percentage = update.percentage ?? current.percentage;
  return {
    ...current,
    ...update,
    percentage: Math.min(100, Math.max(0, percentage)),
  };
}

export function formatFragment(progress: DownloadProgress): string {
  if (progress.currentFragment == null || progress.totalFragments == null) {
    return "-";
  }
  return `${progress.currentFragment}/${progress.totalFragments}`;
}
