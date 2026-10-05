import { expect, test } from "bun:test";
import { installProgress } from "../progress";

test("shows counts and the active plugin, then clears the line and stops", async () => {
  const chunks: string[] = [];
  const progress = installProgress(
    "claude",
    (text) => {
      chunks.push(text);
    },
    () => 100,
  );
  progress.update({ completed: 1, total: 2, label: "Installing ast-grep" });
  expect(chunks.join("")).toContain("1/2 50% · Installing ast-grep");
  await new Promise((resolve) => setTimeout(resolve, 100));
  expect(chunks.length).toBeGreaterThan(1);
  progress.update({ completed: 2, total: 2, label: "Finished" });
  expect(chunks.at(-1)).toContain("2/2 100%");
  progress.stop();
  expect(chunks.at(-1)).toBe("\r\u001b[2K");
  const stopped = chunks.length;
  await new Promise((resolve) => setTimeout(resolve, 100));
  expect(chunks.length).toBe(stopped);
});

test("fits narrow terminals and handles an empty selection", () => {
  const chunks: string[] = [];
  const progress = installProgress(
    "codex",
    (text) => {
      chunks.push(text);
    },
    () => 30,
  );
  try {
    progress.update({ completed: 0, total: 0, label: "Preparing marketplace" });
    const output = chunks.join("");
    expect(output.replace("\r\u001b[2K", "").length).toBeLessThan(30);
    expect(output).not.toContain("NaN");
  } finally {
    progress.stop();
  }
});
