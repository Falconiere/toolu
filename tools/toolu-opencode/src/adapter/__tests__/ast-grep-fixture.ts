/**
 * #347: a temp git project whose data root holds ast-grep's native manifests,
 * driven by the same before/after handlers the plugin
 * wires (`enforcement.ts`). Each call returns the model-visible result text.
 */
import { expect } from "bun:test";
import { execFileSync, spawnSync } from "node:child_process";
import {
  existsSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { delimiter, join } from "node:path";
import { requiredBuiltTooluBinary } from "@toolu/conformance/harness/entry-command";
import { z } from "zod";
import { definedEnv, tooluProcessEnv } from "../../host/runtime-env.ts";
import { createToolAdviceStore } from "../tool-advice.ts";
import { createToolBeforeHandler } from "../tool-before.ts";
import { createToolPostHandler } from "../tool-post.ts";

const REPO_ROOT = join(import.meta.dir, "../../../../..");
const SESSION = "ses_ast-grep";
export const PATTERN = "export function $N($$$) { $$$ }";
export const SEARCH = `ast-grep run -p '${PATTERN}' -l typescript src`;

const LedgerLine = z.object({ kind: z.string(), returned: z.number(), full: z.number() });

export type HostCall = {
  tool: string;
  args: Record<string, unknown>;
  output: unknown;
  metadata?: Record<string, unknown>;
};

export type AstGrepProject = {
  root: string;
  /** Run one call through before, then after; the after output, or the refusal. */
  call: (call: HostCall, id?: string) => Promise<string>;
  /** Re-deliver the after callback of an earlier call. */
  replay: (call: HostCall, id: string) => Promise<string>;
  ledger: () => Array<z.infer<typeof LedgerLine>>;
};

/** PATH with every directory holding `ast-grep` or `sg` replaced by a link farm without them. */
function pathWithoutAstGrep(root: string): string {
  const hidden = new Set(["ast-grep", "sg"]);
  return (process.env.PATH ?? "")
    .split(delimiter)
    .filter((dir) => dir !== "")
    .map((dir, index) => {
      if (![...hidden].some((name) => existsSync(join(dir, name)))) return dir;
      const farm = join(root, ".no-ast-grep", String(index));
      mkdirSync(farm, { recursive: true });
      for (const name of readdirSync(dir).filter((n) => !hidden.has(n))) {
        symlinkSync(join(dir, name), join(farm, name));
      }
      return farm;
    })
    .join(delimiter);
}

export function registerAstGrep(root: string, dataRoot: string): void {
  const env = tooluProcessEnv(definedEnv(process.env), {
    projectRoot: root,
    dataRoot,
    userConfigRoot: join(root, ".xdg/opencode"),
    repoRoot: REPO_ROOT,
  });
  const res = spawnSync(
    env.TOOLU_BIN ?? requiredBuiltTooluBinary(),
    [
      "ast-grep",
      "hook",
      "register",
      "--event",
      "SessionStart",
      "--plugin-root",
      join(REPO_ROOT, "plugins/ast-grep"),
    ],
    { cwd: root, env, input: "{}", encoding: "utf8" },
  );
  expect(res.status).toBe(0);
}

export function astGrepProject(
  options: { astGrep?: "available" | "missing" | "opt-out" } = {},
): AstGrepProject {
  const root = mkdtempSync(join(process.env.TMPDIR ?? "/tmp", "toolu-oc-ast-grep-"));
  execFileSync("git", ["init", "-q"], { cwd: root });
  mkdirSync(join(root, "src"));
  writeFileSync(
    join(root, "src/app.ts"),
    "export function greet(name: string) {\n  return name;\n}\n",
  );
  writeFileSync(join(root, "notes.md"), "TODO: write notes\n");
  if (options.astGrep === "opt-out") {
    mkdirSync(join(root, ".opencode"), { recursive: true });
    writeFileSync(
      join(root, ".opencode/toolu.config.json"),
      JSON.stringify({ version: 1, skills: { "ast-grep": false } }),
    );
  }
  const dataRoot = join(root, ".opencode/toolu/state");
  registerAstGrep(root, dataRoot);
  const path = options.astGrep === "missing" ? pathWithoutAstGrep(root) : (process.env.PATH ?? "");
  const gate = {
    repoRoot: REPO_ROOT,
    configRoot: dataRoot,
    userConfigRoot: join(root, ".xdg/opencode"),
    permissionContext: { cwd: root, projectRoot: root, worktree: root },
    selectedPluginSpecs: new Set(["toolu@toolu", "ast-grep@toolu"]),
    env: { ...definedEnv(process.env), PATH: path, TOOLU_HOST_OVERRIDE: "opencode" },
  };
  const advice = createToolAdviceStore();
  const post = createToolPostHandler(gate, advice);
  const before = createToolBeforeHandler(gate, advice, (c) => post.begin(c));
  let next = 0;
  const after = async (c: HostCall, id: string): Promise<string> => {
    const out = { title: c.tool, output: "", metadata: c.metadata ?? {} };
    // The host's after output is typed text; a broken host can still hand over another value.
    Object.defineProperty(out, "output", { value: c.output, writable: true });
    await post.after({ tool: c.tool, sessionID: SESSION, callID: id, args: c.args }, out);
    return out.output;
  };
  const call = async (c: HostCall, id = `call-${String(next++)}`): Promise<string> => {
    await before({ tool: c.tool, sessionID: SESSION, callID: id }, { args: c.args });
    return after(c, id);
  };
  const ledger = () => {
    const file = join(dataRoot, "toolu/byte-savings/sesast-grep.jsonl");
    if (!existsSync(file)) return [];
    return readFileSync(file, "utf8")
      .trim()
      .split("\n")
      .map((line) => LedgerLine.parse(JSON.parse(line)));
  };
  return { root, call, replay: after, ledger };
}

/** The real stdout of the skill's search example in `root`. */
export function searchOutput(root: string): string {
  const res = spawnSync("ast-grep", ["run", "-p", PATTERN, "-l", "typescript", "src"], {
    cwd: root,
    encoding: "utf8",
  });
  expect(res.status).toBe(0);
  return res.stdout;
}

/** UTF-8 bytes of `text` without trailing newlines, as byte-savings counts them. */
export function bytes(text: string): number {
  return Buffer.byteLength(text.replace(/\n+$/u, ""), "utf8");
}

/** How many times `needle` appears in `text`. */
export function count(text: string, needle: string): number {
  return text.split(needle).length - 1;
}
