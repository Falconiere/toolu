/** Native SessionStart migration over the five shared host cases. */
import { expect, test } from "bun:test";
import { existsSync, lstatSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { BINARY, put, runInOrder } from "../statusline-harness.ts";

const ROOT = resolve(import.meta.dir, "../../../../..");
const Startup = z.strictObject({
  name: z.string(),
  kind: z.literal("startup"),
  host: z.enum(["claude", "codex"]),
  commands: z.array(z.string()).min(1),
  expected: z.enum(["migrated", "silent"]),
});
const Settings = z.object({ statusLine: z.object({ command: z.string() }) });
const cases = readCaseFile(join(ROOT, "fixtures/statusline/cases.json"))
  .filter((raw) => raw.kind === "startup")
  .map((raw) => Startup.parse(raw));

for (const c of cases) {
  test.concurrent(c.name, async () => {
    await runInOrder(c.commands, async (command) => {
      using sb = createSandbox();
      const cfg = sb.path(c.host === "claude" ? "claude-cfg" : "codex-cfg");
      const claude = sb.path("claude-cfg");
      const settings = join(claude, "settings.json");
      const initial = JSON.stringify({ statusLine: { type: "command", command } });
      put(settings, initial);
      const env = {
        HOME: sb.home,
        CLAUDE_CONFIG_DIR: claude,
        CODEX_HOME: cfg,
        TOOLU_HOST_OVERRIDE: c.host,
        PATH: `${dirname(BINARY)}:${process.env.PATH ?? ""}`,
      };
      const argv = [BINARY, "statusline", "hook", "session-start", "--event", "SessionStart"];
      const first = await run(argv, { env, stdin: "{}" });
      expect(first).toMatchObject({ exitCode: 0, stderr: "" });
      const actual: unknown = JSON.parse(readFileSync(settings, "utf8"));
      const parsed = Settings.parse(actual);
      if (c.expected === "migrated") {
        expect(first.stdout).toContain("migrated");
        expect(parsed.statusLine.command).toBe("toolu statusline render");
        expect(readFileSync(`${settings}.bak`, "utf8")).toBe(initial);
      } else {
        expect(first.stdout).toBe("");
        expect(parsed.statusLine.command).toBe(command);
      }
      const path = join(cfg, "statusline/statusline.sh");
      expect(existsSync(path)).toBe(true);
      expect(lstatSync(path).isSymbolicLink()).toBe(true);
      const second = await run(argv, { env, stdin: "{}" });
      expect(second).toMatchObject({ exitCode: 0, stdout: "", stderr: "" });
    });
  });
}
