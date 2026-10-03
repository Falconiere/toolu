/** Native command failure metadata survives durable launch error encoding. */
import { expect, test } from "bun:test";
import { CommandError, commandExitCode, failureDetails, run } from "../common.ts";
import { acknowledgedStatus } from "../lifecycle.ts";

test.concurrent("real nonzero exit and timeout preserve distinct native outcomes", async () => {
  for (const [argv, timeoutMs, expectedCode, uncertain] of [
    [["sh", "-c", "echo native-failure >&2; exit 7"], 5000, 7, false],
    [["sh", "-c", "sleep 5"], 80, null, true],
  ] as const) {
    let failure: unknown;
    try {
      await run([...argv], { timeoutMs });
    } catch (error) {
      failure = error;
    }
    expect(failure).toBeInstanceOf(CommandError);
    const details = failureDetails(failure);
    expect(details.lifecycle_outcome).toBe("uncertain");
    expect(details.native_error).toMatchObject({ uncertain, code: null });
    if (expectedCode !== null) {
      expect(details.native_error).toMatchObject({ exitCode: expectedCode });
      expect(commandExitCode(failure)).toBe(7);
      expect(details.lifecycle_error).toContain("native-failure");
    }
  }
});

test.concurrent("blocked and provider-limited evidence stays distinct from successful start", () => {
  expect(acknowledgedStatus("blocked", "Approval required", "started")).toBe("blocked");
  expect(
    acknowledgedStatus("blocked", "You've hit your usage limit. Try again at 5pm", "resumed"),
  ).toBe("provider-limited");
  expect(acknowledgedStatus("working", "", "resumed")).toBe("resumed");
  expect(
    failureDetails(new CommandError("quota exceeded", 7, false, "provider_error")),
  ).toMatchObject({
    lifecycle_outcome: "provider-limited",
    native_error: { exitCode: 7, code: "provider_error", uncertain: false },
  });
  expect(commandExitCode(new CommandError("timeout", 0, true))).toBe(1);
});
