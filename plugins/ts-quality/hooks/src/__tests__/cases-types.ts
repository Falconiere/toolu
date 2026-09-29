/** The shape of a ts-quality golden case (#265) and the project presets the bats suites used. */
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

export type TsCase = {
  readonly name: string;
  /** Default Claude only. */
  readonly hosts?: readonly PretoolHost[];
  /** Committed project files; default `TS_PROJECT`. */
  readonly project: Readonly<Record<string, string>>;
  /** More committed files. */
  readonly commit?: Readonly<Record<string, string>>;
  /** Project `toolu.config.json`. */
  readonly config?: object;
  /** Other plugins registered by their real `register.sh`. */
  readonly register?: readonly string[];
  readonly env?: (sb: Sandbox) => EnvPatch;
  readonly setup?: (sb: Sandbox) => void;
  readonly steps: readonly Step[];
  /** Decision class of the last step. */
  readonly expect: PostOutcome;
  /** Text the last step's output contains, and must not contain (the bats assertions). */
  readonly contains?: readonly string[];
  readonly absent?: readonly string[];
  /**
   * The excerpt order is not part of the contract: bash printed ast-grep's
   * multi-rule hits unsorted, and that order varies run to run.
   */
  readonly unordered?: boolean;
};

/** `_ts_project` in the bats suites: tsconfig, package.json and a bun lock file. */
export const TS_PROJECT = {
  "tsconfig.json": "{}\n",
  "package.json": '{"name":"x"}\n',
  "bun.lock": "",
};

/** `_ts_project` of assembled.bats: the tsconfig declares a `@/*` path alias. */
export const ALIAS_PROJECT = {
  ...TS_PROJECT,
  "tsconfig.json": '{"compilerOptions":{"paths":{"@/*":["./src/*"]}}}\n',
};

/** A single Write of `file` with `body`. */
export function wrote(file: string, body: string, tool: Step["tool"] = "Write"): Step[] {
  return [{ write: { [file]: body }, tool, file }];
}

/** `n` lines of `export const vI = I;`. */
export function constLines(n: number): string {
  return Array.from(
    { length: n },
    (_, i) => `export const v${String(i + 1)} = ${String(i + 1)};\n`,
  ).join("");
}
