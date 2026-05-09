import { describe, expect, it } from "vitest";
import { deriveFilename, isValidHttpUrl, parseResolutionLabel } from "../src/lib/m3u8";

describe("m3u8 helpers", () => {
  it.each([
    ["https://cdn.example.com/video/720p/index.m3u8?token=abc", "720p"],
    ["https://cdn.example.com/video/1080p/index.m3u8?token=def", "1080p"],
    ["https://cdn.example.com/video/stream.m3u8?token=ghi", "Unknown resolution"],
  ])("parses resolution from %s", (url, label) => {
    expect(parseResolutionLabel(url)).toBe(label);
  });

  it("validates only http and https URLs", () => {
    expect(isValidHttpUrl("https://example.com/watch/123")).toBe(true);
    expect(isValidHttpUrl("file:///tmp/movie")).toBe(false);
    expect(isValidHttpUrl("not a url")).toBe(false);
  });

  it("derives mp4 filenames from URL paths", () => {
    expect(deriveFilename("https://example.com/watch/my-video.html")).toBe("my-video.mp4");
  });
});
