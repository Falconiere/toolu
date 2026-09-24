import { describe, expect, test } from "bun:test";
import { CANCEL_SYMBOL } from "@clack/prompts";
import { CliError, EXIT } from "../../exit";
import { rejectIfCancelled, selectHost, selectHosts, selectPlugins } from "../prompts";

describe("rejectIfCancelled", () => {
  test("returns a concrete value unchanged", () => {
    expect(rejectIfCancelled("claude")).toBe("claude");
    expect(rejectIfCancelled(["claude", "codex"])).toEqual(["claude", "codex"]);
  });

  test("maps the Clack cancel sentinel to exit 130", () => {
    expect(() => rejectIfCancelled(CANCEL_SYMBOL)).toThrow(CliError);
    try {
      rejectIfCancelled(CANCEL_SYMBOL);
    } catch (error) {
      expect(error).toBeInstanceOf(CliError);
      expect((error as CliError).code).toBe(EXIT.cancelled);
      expect((error as CliError).message).toBe("cancelled");
    }
  });
});

describe("prompt exports", () => {
  test("selectHosts, selectHost, and selectPlugins are functions", () => {
    expect(typeof selectHosts).toBe("function");
    expect(typeof selectHost).toBe("function");
    expect(typeof selectPlugins).toBe("function");
  });
});
