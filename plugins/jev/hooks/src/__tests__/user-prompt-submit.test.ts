import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { entryArgv } from "@toolu/conformance/harness/entry-command";

const plugin = join(import.meta.dir, "../../..");
const startup = "session-start";
const prompt = "user-prompt-submit";

async function run(entry: string, env: Record<string, string | undefined>, stdin: string) {
  const child = Bun.spawn(entryArgv("jev", entry, plugin), {
    env,
    stdin: "pipe",
    stdout: "pipe",
    stderr: "pipe",
  });
  child.stdin.write(stdin);
  child.stdin.end();
  const [stdout, stderr, status] = await Promise.all([
    new Response(child.stdout).text(),
    new Response(child.stderr).text(),
    child.exited,
  ]);
  return { stdout, stderr, status };
}

test("prompt hook names the published wrapper for a task and stays silent for confirmations", async () => {
  const root = mkdtempSync(join(tmpdir(), "jev-prompt-"));
  try {
    const env = {
      ...process.env,
      HOME: root,
      CLAUDE_CONFIG_DIR: join(root, "claude"),
      TOOLU_CONFIG_DIR: join(root, "config"),
      TYPESAFE_API_KEY: "fixture-key",
    };
    expect((await run(startup, env, "{}")).status).toBe(0);
    const task = await run(prompt, env, JSON.stringify({ prompt: "rank these approaches" }));
    expect(task.status).toBe(0);
    const json = JSON.parse(task.stdout);
    expect(json.hookSpecificOutput.hookEventName).toBe("UserPromptSubmit");
    expect(json.hookSpecificOutput.additionalContext).toContain(
      `${env.TOOLU_CONFIG_DIR}/jev/jev.sh`,
    );
    expect((await run(prompt, env, JSON.stringify({ prompt: "LGTM" }))).stdout).toBe("");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("Codex path, slash command, multiline task and missing key match the prompt contract", async () => {
  const root = mkdtempSync(join(tmpdir(), "jev-codex-prompt-"));
  try {
    const env: Record<string, string | undefined> = {
      ...process.env,
      HOME: root,
      CODEX_HOME: join(root, "codex profile"),
      TOOLU_HOST_OVERRIDE: "codex",
      TYPESAFE_API_KEY: "fixture-key",
    };
    expect((await run(startup, env, "{}")).status).toBe(0);
    const slash = await run(
      prompt,
      env,
      JSON.stringify({ prompt: "/delivery-flow:delivery-flow ship export" }),
    );
    expect(JSON.parse(slash.stdout).hookSpecificOutput.additionalContext).toContain(
      `${env.CODEX_HOME}/jev/jev.sh`,
    );
    const multiline = await run(prompt, env, JSON.stringify({ prompt: "ok\nnow add pagination" }));
    expect(JSON.parse(multiline.stdout).hookSpecificOutput.hookEventName).toBe("UserPromptSubmit");
    expect((await run(prompt, env, JSON.stringify({ prompt: "\n\ty\t\n" }))).stdout).toBe("");
    delete env.TYPESAFE_API_KEY;
    env.PATH = "/nonexistent";
    const check = await run(prompt, env, JSON.stringify({ prompt: "rank these" }));
    expect(check.stderr).toBe("");
    const context = JSON.parse(check.stdout).hookSpecificOutput.additionalContext;
    expect(context).toContain(process.execPath);
    expect(context).toContain("The Jev hook did not receive TYPESAFE_API_KEY");
    expect(context).toContain("command environment without printing its value");
    expect(context).not.toContain("Jev unavailable (missing:");
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test("an unpublished wrapper and invalid prompt input are silent", async () => {
  const root = mkdtempSync(join(tmpdir(), "jev-unpublished-"));
  try {
    const env = {
      ...process.env,
      HOME: root,
      TOOLU_CONFIG_DIR: join(root, "config"),
      TYPESAFE_API_KEY: "fixture-key",
    };
    for (const input of ["", "bad json", "{}", JSON.stringify({ prompt: "rank these" })]) {
      const runResult = await run(prompt, env, input);
      expect(runResult).toEqual({ status: 0, stdout: "", stderr: "" });
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
