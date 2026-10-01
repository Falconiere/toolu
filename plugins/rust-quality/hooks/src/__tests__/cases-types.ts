/** The shape of a rust-quality golden case (#267) and the project preset the bats suites used. */
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

export type RsCase = {
  readonly name: string;
  /** The bats tests this case ports, as `<suite>.bats: <title>`. */
  readonly from?: readonly string[];
  /** Default Claude only. */
  readonly hosts?: readonly PretoolHost[];
  /** Committed project files; `RUST_PROJECT` in most cases. */
  readonly project: Readonly<Record<string, string>>;
  /** More committed files. */
  readonly commit?: Readonly<Record<string, string>>;
  /** Project `toolu.config.json`. */
  readonly config?: object;
  /** Other plugins registered by their real register entry. */
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

/** `_rust_project` in the bats suites: a `Cargo.toml` at the root. */
export const RUST_PROJECT = { "Cargo.toml": '[package]\nname = "x"\nversion = "0.1.0"\n' };

/** A single Write of `file` with `body`. */
export function wrote(file: string, body: string, tool: Step["tool"] = "Write"): Step[] {
  return [{ write: { [file]: body }, tool, file }];
}

/** `n` lines of `pub const VI: u32 = I;`. */
export function constLines(n: number): string {
  return Array.from(
    { length: n },
    (_, i) => `pub const V${String(i + 1)}: u32 = ${String(i + 1)};\n`,
  ).join("");
}

/** A bats `from` reference. */
export function bats(suite: string, ...titles: string[]): string[] {
  return titles.map((title) => `${suite}.bats: ${title}`);
}
