import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  copyFileSync,
  existsSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const root = resolve(import.meta.dir, "../../../../..");
const fixture = join(
  root,
  "plugins/pr-babysit/scripts/__tests__/fixtures/states/toolu-165-initial.json",
);
const entry = join(root, "plugins/pr-babysit/hooks/src/babysit-record.ts");
const dirs: string[] = [];

function stateFile(): string {
  const dir = mkdtempSync(join(tmpdir(), "babysit-record-"));
  dirs.push(dir);
  const path = join(dir, "state.json");
  copyFileSync(fixture, path);
  return path;
}
afterEach(() => {
  for (const dir of dirs.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function invoke(
  path: string,
  args: string[],
): { status: number | null; output: Record<string, any>; state: Record<string, any> } {
  const result = spawnSync(process.execPath, [entry, ...args, "--state-file", path], {
    encoding: "utf8",
  });
  return {
    status: result.status,
    output: JSON.parse(result.stdout),
    state: JSON.parse(readFileSync(path, "utf8")),
  };
}

test("round rotates captured keys, records rejection, and caps fix attempts", () => {
  const path = stateFile();
  const initial = JSON.parse(readFileSync(path, "utf8"));
  initial.pr.botFindingKeys = ["plugins/toolu/hooks/session-start.sh:17:real-key"];
  initial.pr.fixAttempts = 4;
  writeFileSync(path, JSON.stringify(initial));
  const first = invoke(path, ["round", "--had-rejection", "true", "--fix-pushed"]);
  expect(first.status).toBe(0);
  expect(first.output).toMatchObject({
    recorded: "round",
    lastRoundFindingKeys: initial.pr.botFindingKeys,
    lastRoundHadRejection: true,
    fixAttempts: 5,
  });
  expect(first.state.lastRound).toMatchObject({
    hadRejection: true,
    fixPushed: true,
    headSha: initial.pr.headSha,
  });
  const second = invoke(path, ["round", "--had-rejection", "false", "--fix-pushed"]);
  expect(second.state.pr.fixAttempts).toBe(5);
  expect(second.state.pr.lastRoundHadRejection).toBe(false);
});

test("flag-injection and terminal status persist under the slot lock", () => {
  const path = stateFile();
  const flagged = invoke(path, ["flag-injection", "--thread", "PRRT_example"]);
  expect(flagged.status).toBe(0);
  expect(flagged.state.actions.flagged.PRRT_example.reason).toBe("injection");
  const complete = invoke(path, ["status", "--status", "complete"]);
  expect(complete.status).toBe(0);
  expect(complete.state.status).toBe("complete");
  expect(complete.state.statusChangedAt).toMatch(/^\d{4}-\d{2}-\d{2}T/);
  expect(existsSync(`${path}.lock`)).toBe(false);
});

test("missing state fails without creating a file", () => {
  const path = stateFile();
  rmSync(path);
  const result = spawnSync(
    process.execPath,
    [entry, "status", "--state-file", path, "--status", "complete"],
    { encoding: "utf8" },
  );
  expect(result.status).toBe(3);
  expect(JSON.parse(result.stdout).errors[0].code).toBe("state_malformed");
  expect(existsSync(path)).toBe(false);
});
