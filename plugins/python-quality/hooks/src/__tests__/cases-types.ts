/** The shape of a python-quality golden case (#266) and the project the bats suites used. */
import type { PatchFile } from "@toolu/conformance/harness/fixtures";
import type { PostOutcome } from "@toolu/conformance/harness/posttool-corpus";
import type { PretoolHost } from "@toolu/conformance/harness/pretool";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";
import type { EnvPatch } from "@toolu/conformance/harness/spawn";

/** One tool call, after its files are written (or removed) on disk. */
export type Step = {
  /** Files written before the call, relative to the project. */
  readonly write?: Readonly<Record<string, string>>;
  readonly remove?: readonly string[];
  /** `Delete`: an Edit with `TOOLU_EDIT_OPERATION=delete` on Claude, a delete patch on Codex. */
  readonly tool?: "Write" | "Edit" | "MultiEdit" | "Bash" | "Delete";
  /** The edited file, relative to the project; the payload carries it absolute unless `relative`. */
  readonly file?: string;
  readonly relative?: boolean;
  /** Extra `tool_input` members. */
  readonly input?: Readonly<Record<string, unknown>>;
  /** A Codex `apply_patch` over several paths instead of one tool call. */
  readonly patch?: readonly PatchFile[];
  /** A Codex `apply_patch` command given verbatim (moves). */
  readonly rawPatch?: string;
  readonly env?: EnvPatch;
};

export type PyCase = {
  readonly name: string;
  /** Default Claude only. */
  readonly hosts?: readonly PretoolHost[];
  /** Committed project files. */
  readonly project: Readonly<Record<string, string>>;
  /** Project `toolu.config.json`. */
  readonly config?: object;
  /** Other plugins registered by their real register hook. */
  readonly register?: readonly string[];
  readonly env?: (sb: Sandbox) => EnvPatch;
  readonly setup?: (sb: Sandbox) => void;
  readonly steps: readonly Step[];
  /** Decision class of the last step. */
  readonly expect: PostOutcome;
  /** Text the last step's output contains, and must not contain (the bats assertions). */
  readonly contains?: readonly string[];
  readonly absent?: readonly string[];
};

/** `_python_project` in the bats suites. */
export const PY_PROJECT = { "pyproject.toml": '[project]\nname = "x"\nversion = "0.1.0"\n' };

/** A single call of `tool` on `file` after writing `body`. */
export function wrote(file: string, body: string, tool: Step["tool"] = "Write"): Step[] {
  return [{ write: { [file]: body }, tool, file }];
}

/** `for i in $(seq from to); do echo "<indent>v$i = $i"; done`. */
export function assignments(from: number, to: number, indent = ""): string {
  const out: string[] = [];
  for (let i = from; i <= to; i++) out.push(`${indent}v${String(i)} = ${String(i)}\n`);
  return out.join("");
}

/** A case in the bats `_python_project`, one step, Claude only. */
export function pyCase(
  name: string,
  steps: Step[],
  check: Pick<PyCase, "contains" | "absent" | "expect">,
  extra: Partial<PyCase> = {},
): PyCase {
  return { name, project: PY_PROJECT, steps, ...check, ...extra };
}
