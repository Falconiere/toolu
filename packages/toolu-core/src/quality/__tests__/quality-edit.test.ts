/** Edited-file extraction and linked-worktree checks over shared JSON cases. */
import { expect, test } from "bun:test";
import { materializeCaseValue, applyCaseSetup } from "@toolu/conformance/harness/json-cases";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { editedFile, inLinkedWorktree, isRegularFile } from "../quality-edit.ts";
import { postContext, postEvent } from "./quality-harness.ts";
import { EDIT_CASES, materializeRunnerCall } from "./runner-cases.ts";

for (const c of EDIT_CASES) {
  test.concurrent(c.name, () => {
    using sb = createSandbox({ git: true });
    applyCaseSetup(sb, c.setup);
    for (const check of c.checks) {
      if (check.subject === "regular") {
        expect(
          isRegularFile({ path: check.path, absolute: sb.path(check.path), removed: false }),
        ).toBe(check.expected);
        continue;
      }
      if (check.subject === "linked") {
        const path = z.string().parse(materializeCaseValue(sb, check.path));
        const call = { input: { file_path: path } };
        const ctx = postContext(sb, call);
        const file = editedFile(postEvent(sb, call), ctx);
        expect(file).toBeDefined();
        if (file === undefined) throw new Error(`${c.name}: no edited file for ${path}`);
        expect(inLinkedWorktree(file, ctx)).toBe(check.expected);
        continue;
      }
      const call = materializeRunnerCall(sb, check.call);
      const file = editedFile(postEvent(sb, call), postContext(sb, call));
      switch (check.field) {
        case "undefined":
          expect(file).toBeUndefined();
          break;
        case "path":
          expect<unknown>(file?.path).toBe(check.expected);
          break;
        case "removed":
          expect<unknown>(file?.removed).toBe(check.expected);
          break;
        case "full":
          expect<unknown>(
            file === undefined
              ? undefined
              : { ...file, absolute: file.absolute.replace(sb.project, "<P>") },
          ).toEqual(check.expected);
          break;
      }
    }
  });
}
