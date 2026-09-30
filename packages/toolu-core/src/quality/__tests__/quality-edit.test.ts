/**
 * The edited file (#265): where `FILE_PATH` comes from, when a path counts as
 * removed, and the linked-worktree skip, over real sandbox repositories.
 */
import { expect, test } from "bun:test";
import { join, relative } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { editedFile, inLinkedWorktree, isRegularFile } from "../quality-edit.ts";
import { postContext, postEvent, type Call } from "./quality-harness.ts";

function resolveFile(call: Call) {
  using sb = createSandbox({ git: true });
  const file = editedFile(postEvent(sb, call), postContext(sb, call));
  return file === undefined
    ? undefined
    : { ...file, absolute: file.absolute.replace(sb.project, "<P>") };
}

test("an edit tool's path, then file_path, then target_file", () => {
  expect(resolveFile({ input: { file_path: "src/a.ts", path: "src/p.ts" } })?.path).toBe(
    "src/p.ts",
  );
  expect(resolveFile({ input: { file_path: "src/a.ts" } })).toEqual({
    path: "src/a.ts",
    absolute: "<P>/src/a.ts",
    removed: false,
  });
  expect(resolveFile({ toolName: "MultiEdit", input: { target_file: "/abs/t.ts" } })?.path).toBe(
    "/abs/t.ts",
  );
});

test("jq's // skips null and false but keeps an empty string", () => {
  expect(resolveFile({ input: { path: null, file_path: false, target_file: "t.ts" } })?.path).toBe(
    "t.ts",
  );
  expect(resolveFile({ input: { path: "", file_path: "a.ts" } })).toBeUndefined();
});

test("CLAUDE_FILE_PATHS wins, for any tool; a non-edit tool alone names nothing", () => {
  const env = { CLAUDE_FILE_PATHS: "env.ts" };
  expect(resolveFile({ input: { file_path: "a.ts" }, env })?.path).toBe("env.ts");
  expect(resolveFile({ toolName: "Bash", input: { file_path: "a.ts" }, env })?.path).toBe("env.ts");
  expect(resolveFile({ toolName: "Bash", input: { file_path: "a.ts" } })).toBeUndefined();
});

test("a delete or a move source is removed, from the split edit, the env or tool_input", () => {
  const input = { file_path: "a.ts" };
  const split = (operation: "update" | "delete", movedTo: string): Call => ({
    input,
    edit: { operation, from: "a.ts", movedTo },
  });
  expect(resolveFile(split("delete", ""))?.removed).toBe(true);
  expect(resolveFile(split("update", "b.ts"))?.removed).toBe(true);
  expect(resolveFile(split("update", ""))?.removed).toBe(false);
  expect(resolveFile({ input, env: { TOOLU_EDIT_OPERATION: "delete" } })?.removed).toBe(true);
  expect(resolveFile({ input: { ...input, toolu_edit_moved_to: "b.ts" } })?.removed).toBe(true);
  // With a split edit the inherited variable is replaced by the split's own value.
  const replaced = { ...split("update", ""), env: { TOOLU_EDIT_OPERATION: "delete" } };
  expect(resolveFile(replaced)?.removed).toBe(false);
});

test("a regular file counts; a directory or a missing path does not", () => {
  using sb = createSandbox({ git: true });
  sb.write("src/a.ts", "x\n");
  const at = (path: string) => ({ path, absolute: sb.path(path), removed: false });
  expect(isRegularFile(at("src/a.ts"))).toBe(true);
  expect(isRegularFile(at("src"))).toBe(false);
  expect(isRegularFile(at("src/none.ts"))).toBe(false);
});

test("edited file paths inside a linked worktree are skipped", () => {
  using sb = createSandbox({ git: true });
  const linked = join(sb.root, "linked");
  sb.git("worktree", "add", "-q", "-b", "side", linked);
  sb.write("src/a.ts", "x\n");
  const check = (path: string) => {
    const call = { input: { file_path: path } };
    const ctx = postContext(sb, call);
    const file = editedFile(postEvent(sb, call), ctx);
    expect(file).toBeDefined();
    return file === undefined ? false : inLinkedWorktree(file, ctx);
  };
  expect(check(join(linked, "a.ts"))).toBe(true);
  expect(check(relative(sb.project, join(linked, "a.ts")))).toBe(true);
  expect(check(sb.path("src/a.ts"))).toBe(false);
  expect(check("src/a.ts")).toBe(false);
  expect(check(join(sb.home, "a.ts"))).toBe(false);
});
