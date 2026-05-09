import { describe, expect, it } from "vitest";
import { nextTaskState } from "../src/lib/taskState";

describe("task state matrix", () => {
  it("pauses a downloading task", () => {
    expect(nextTaskState("Downloading", "pause")).toBe("Paused");
  });

  it("rejects pause after completion", () => {
    expect(nextTaskState("Completed", "pause")).toBeNull();
  });

  it.each([
    ["Pending", "cancel", "Cancelled"],
    ["Downloading", "relay", "Relaying"],
    ["Paused", "resume", "Downloading"],
    ["Relaying", "download", "Downloading"],
  ] as const)("handles %s + %s", (state, action, next) => {
    expect(nextTaskState(state, action)).toBe(next);
  });
});
