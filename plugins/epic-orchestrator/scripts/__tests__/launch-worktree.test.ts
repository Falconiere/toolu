import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";

const SCRIPT = join(import.meta.dir, "..", "launch-worktree.ts");

test.concurrent("a listed worktree without a path is refused, never resolved as ''", async () => {
  using sb = createSandbox();
  const bin = join(sb.root, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(
    join(bin, "herdr"),
    [
      "#!/bin/sh",
      'case "$1 $2" in',
      `  "worktree list") echo '{"worktrees":[{"branch":"feat/x","open_workspace_id":"w1"}]}' ;;`,
      `  "pane list") echo '{"panes":[{"pane_id":"p1"}]}' ;;`,
      "  *) exit 9 ;;",
      "esac",
    ].join("\n"),
  );
  chmodSync(join(bin, "herdr"), 0o755);
  const driver = join(sb.root, "driver.ts");
  writeFileSync(
    driver,
    [
      `import { ensureWorktree } from ${JSON.stringify(SCRIPT)};`,
      "const issue = { ref: 'x', key: 'x-1', url: '', title: '', repo: 'o/r', number: 1, status: 'ready',",
      "  open_blockers: [], blockers: {}, branch: 'feat/x', checkout: null };",
      `await ensureWorktree(${JSON.stringify(sb.project)}, issue, "main", false, []);`,
    ].join("\n"),
  );
  const proc = Bun.spawn([process.execPath, driver], {
    env: { ...process.env, PATH: `${bin}:${process.env.PATH ?? ""}` },
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stderr, code] = await Promise.all([new Response(proc.stderr).text(), proc.exited]);
  expect(code).not.toBe(0);
  expect(stderr).toContain("herdr listed feat/x without a worktree path");
});
