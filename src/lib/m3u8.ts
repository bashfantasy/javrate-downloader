const resolutionPattern = /(?:^|[^\d])((?:2160|1440|1080|720|540|480|360|240)p)(?:[^\d]|$)/i;

export function parseResolutionLabel(url: string): string {
  const decoded = safeDecode(url);
  const match = decoded.match(resolutionPattern);
  return match ? match[1].toLowerCase() : "Unknown resolution";
}

export function deriveFilename(pageUrl: string): string {
  try {
    const url = new URL(pageUrl);
    const lastSegment = url.pathname.split("/").filter(Boolean).pop() ?? "video";
    const clean = lastSegment.replace(/\.[a-z0-9]+$/i, "").replace(/[^a-zA-Z0-9._-]+/g, "-");
    return `${clean || "video"}.mp4`;
  } catch {
    return "video.mp4";
  }
}

export function isValidHttpUrl(value: string): boolean {
  try {
    const url = new URL(value);
    return url.protocol === "http:" || url.protocol === "https:";
  } catch {
    return false;
  }
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}
