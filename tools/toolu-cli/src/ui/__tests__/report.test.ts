import { describe, expect, test } from "bun:test";
import { anyFailed, reportInstall, reportList, reportRemove, reportUpdate } from "../report";

describe("reportInstall", () => {
  const steps = [
    { name: "toolu", outcome: "installed" as const, detail: "installed", argv: ["claude", "x"] },
    {
      name: "jira",
      outcome: "skew" as const,
      detail: "installed at 6.4.0, marketplace offers 6.5.0",
      argv: ["claude", "y"],
    },
    { name: "jev", outcome: "failed" as const, detail: "boom", argv: ["claude", "z"] },
  ];

  test("names every plugin and its outcome", () => {
    const text = reportInstall(steps, false);
    expect(text).toContain("toolu");
    expect(text).toContain("installed at 6.4.0, marketplace offers 6.5.0");
    expect(text).toContain("boom");
  });

  test("a dry run prints the command lines and says nothing ran", () => {
    const text = reportInstall(steps, true);
    expect(text).toContain("nothing was run");
    expect(text).toContain("claude x");
    expect(text).not.toContain("installed at 6.4.0");
  });

  test("an empty plan renders without throwing", () => {
    expect(reportInstall([], false)).toBe("");
  });
});

describe("anyFailed", () => {
  test("is true for a failed install step and for a failed removal", () => {
    expect(anyFailed([{ outcome: "failed" }])).toBe(true);
    expect(anyFailed([{ removed: false }])).toBe(true);
  });

  test("is false when every step succeeded or was already satisfied", () => {
    expect(anyFailed([{ outcome: "installed" }, { outcome: "already" }])).toBe(false);
    expect(anyFailed([{ removed: true }])).toBe(false);
    expect(anyFailed([])).toBe(false);
  });

  test("a skew is not a failure, because the plugin is still installed", () => {
    expect(anyFailed([{ outcome: "skew" }])).toBe(false);
  });
});

describe("the other reporters", () => {
  test("list distinguishes installed, disabled and absent", () => {
    const text = reportList([
      { name: "toolu", installed: true, version: "6.5.0", enabled: true },
      { name: "jev", installed: true, version: "6.5.0", enabled: false },
      { name: "jira", installed: false, version: undefined, enabled: false },
    ]);
    expect(text).toContain("6.5.0");
    expect(text).toContain("(disabled)");
    expect(text).toContain("not installed");
  });

  test("remove and update name each plugin", () => {
    expect(reportRemove([{ name: "jira", removed: true, detail: "removed", argv: [] }])).toContain(
      "jira",
    );
    expect(
      reportUpdate([{ name: "jira", outcome: "current", detail: "current at 6.5.0", argv: [] }]),
    ).toContain("current at 6.5.0");
  });
});
