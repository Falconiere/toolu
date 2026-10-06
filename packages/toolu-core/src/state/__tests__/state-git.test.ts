/** Branch and slug state on real repositories from shared JSON cases. */
import { expect, test } from "bun:test";
import { resolve } from "node:path";
import { readCaseFile } from "@toolu/conformance/harness/json-cases";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { z } from "zod";
import { baseBranch, branchSlug, branchSlugs, currentBranch } from "../state-git.ts";

const SlugSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("branch-slug"),
  branch: z.string(),
  expected: z.string(),
});
const BaseSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("base-branch"),
  initial: z.string(),
  before: z.string(),
  remote: z.string(),
  after: z.string(),
});
const CurrentSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("current-branch"),
  branch: z.string(),
  expected: z.tuple([z.string(), z.string(), z.string(), z.string()]),
});
const SlugsSchema = z.strictObject({
  name: z.string(),
  kind: z.literal("branch-slugs"),
  merged: z.string(),
  ahead: z.string(),
  file: z.string(),
  body: z.string(),
  base: z.string(),
  expectedAll: z.array(z.string()),
  expectedMerged: z.array(z.string()),
  missingBase: z.string(),
});
const cases = readCaseFile(resolve(import.meta.dir, "../../../../../fixtures/state/cases.json"));

for (const raw of cases) {
  if (raw.kind === "branch-slug") {
    const c = SlugSchema.parse(raw);
    test(c.name, () => expect(branchSlug(c.branch)).toBe(c.expected));
  } else if (raw.kind === "base-branch") {
    const c = BaseSchema.parse(raw);
    test(c.name, () => {
      using sb = createSandbox({ git: true, branch: c.initial });
      expect(baseBranch(sb.project, process.env)).toBe(c.before);
      sb.git("update-ref", c.remote, "HEAD");
      sb.git("symbolic-ref", "refs/remotes/origin/HEAD", c.remote);
      expect(baseBranch(sb.project, process.env)).toBe(c.after);
    });
  } else if (raw.kind === "current-branch") {
    const c = CurrentSchema.parse(raw);
    test(c.name, () => {
      using repo = createSandbox({ git: true, branch: c.branch });
      expect(currentBranch(repo.project, process.env)).toBe(c.expected[0]);
      repo.git("checkout", "-q", "--detach");
      expect(currentBranch(repo.project, process.env)).toBe(c.expected[1]);
      using unborn = createSandbox();
      unborn.git("init", "-q");
      expect(currentBranch(unborn.project, process.env)).toBe(c.expected[2]);
      using plain = createSandbox();
      expect(currentBranch(plain.project, process.env)).toBe(c.expected[3]);
    });
  } else if (raw.kind === "branch-slugs") {
    const c = SlugsSchema.parse(raw);
    test(c.name, () => {
      using sb = createSandbox({ git: true });
      sb.git("branch", c.merged);
      sb.git("checkout", "-q", "-b", c.ahead);
      sb.write(c.file, c.body);
      sb.git("add", c.file);
      sb.git("commit", "-q", "-m", "ahead");
      expect([...branchSlugs(sb.project, process.env)].sort()).toEqual(c.expectedAll);
      expect([...branchSlugs(sb.project, process.env, c.base)].sort()).toEqual(c.expectedMerged);
      expect(branchSlugs(sb.project, process.env, c.missingBase).size).toBe(0);
    });
  }
}
