/**
 * PreToolUse corpus case shape and sandbox helpers (#258), shared by the case
 * lists in `pretool-cases-*.ts` and by `pretool-corpus.ts`.
 */
import { mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";
import type { Fixture } from "./fixtures.ts";
import type { PretoolHost } from "./pretool.ts";
import type { Sandbox } from "./sandbox.ts";

export type Outcome = "deny" | "ask" | "advisory" | "silent" | "exit2";

export type PretoolCase = {
  name: string;
  fixture: (sb: Sandbox) => Fixture;
  /** Raw stdin instead of the rendered fixture (malformed input). */
  stdin?: string;
  /** Project `toolu.config.json` (in `.claude/` or `.codex/`). */
  config?: object;
  /** Lines appended to a copy of `plugins/toolu/settings/<file>`. */
  settings?: Record<string, string>;
  setup?: (sb: Sandbox, host: PretoolHost) => void;
  expect: { claude: Outcome; codex?: Outcome };
};

export function stateDir(host: PretoolHost): string {
  return host === "codex" ? ".codex" : ".claude";
}

export function writeFile(path: string, body: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, body);
}

/** `feat/example` with one committed change on top of `main`. */
export function featureBranch(sb: Sandbox, file = "src/a.ts"): void {
  sb.git("checkout", "-q", "-b", "feat/example");
  sb.write(file, "export const a = 1;\n");
  sb.git("add", "-A");
  sb.git("commit", "-q", "-m", "feat: example");
}

export function mode(gate: string, value: string): object {
  return { version: 1, gates: { [gate]: { mode: value } } };
}

export const abs = (rel: string) => (sb: Sandbox) => sb.path(rel);
