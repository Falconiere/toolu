/**
 * Tests for tooling/shellcheck.sh.
 *
 * The script's whole reason to exist is the second pass: concern fragments are
 * partials of one assembled script, so linting them individually reports noise
 * (no shebang, cross-fragment variables "unassigned"/"unused") while linting the
 * assembled module reports what actually runs. These tests drive the real script
 * against a real fixture tree — no stubbing of shellcheck.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, symlinkSync } from "node:fs";
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";

const ROOT = resolve(import.meta.dir, "../../..");
const SCRIPT = resolve(ROOT, "tooling/shellcheck.sh");
const NO_SHELLCHECK = Bun.which("shellcheck") === null;
const TIMEOUT_MS = 60_000;

/** The fixture tree root holds a copy of the real script, so it lints only the fixture. */
function fixtureTree(sb: Sandbox): string {
  for (const dir of ["tooling", "plugins/demo/hooks/concerns", "benchmarks", ".claude-plugin"]) {
    mkdirSync(sb.path(dir), { recursive: true });
  }
  return sb.write("tooling/shellcheck.sh", readFileSync(SCRIPT, "utf8"));
}

/**
 * A pair of fragments that is only coherent once concatenated: 00 assigns the
 * variable and carries the shebang, 10 consumes it. Linted separately, 10 raises
 * SC2148 (no shebang) and SC2154 (COUNT unassigned).
 */
function splitFragments(sb: Sandbox): void {
  sb.write("plugins/demo/hooks/concerns/00-preamble.sh", "#!/usr/bin/env bash\nCOUNT=3\n");
  sb.write(
    "plugins/demo/hooks/concerns/10-use.sh",
    "if [ \"$COUNT\" -gt 2 ]; then\n  printf 'many\\n'\nfi\n",
  );
}

test.concurrent.skipIf(NO_SHELLCHECK)(
  "shellcheck.sh: passes on a clean tree",
  async () => {
    using sb = createSandbox();
    const script = fixtureTree(sb);
    splitFragments(sb);
    sb.write("plugins/demo/ok.sh", "#!/usr/bin/env bash\nprintf 'hello\\n'\n");
    const res = await run(["bash", script], { cwd: sb.project });
    expect(res.exitCode).toBe(0);
  },
  TIMEOUT_MS,
);

test.concurrent.skipIf(NO_SHELLCHECK)(
  "shellcheck.sh: fragments are linted assembled, not individually",
  async () => {
    // Guards the design: run the SAME fragments through plain per-file shellcheck
    // and assert it reports the noise this script exists to avoid, then assert the
    // script itself stays green on them.
    using sb = createSandbox();
    const script = fixtureTree(sb);
    splitFragments(sb);
    const plain = await run(
      ["shellcheck", "-S", "warning", sb.path("plugins/demo/hooks/concerns/10-use.sh")],
      { cwd: sb.project },
    );
    expect(plain.exitCode).not.toBe(0);
    expect(plain.stdout + plain.stderr).toContain("SC2148");

    const assembled = await run(["bash", script], { cwd: sb.project });
    expect(assembled.exitCode).toBe(0);
  },
  TIMEOUT_MS,
);

test.concurrent.skipIf(NO_SHELLCHECK)(
  "shellcheck.sh: fails on a warning-level defect in a standalone script",
  async () => {
    using sb = createSandbox();
    const script = fixtureTree(sb);
    splitFragments(sb);
    sb.write(
      "plugins/demo/bad.sh",
      [
        "#!/usr/bin/env bash",
        "# SC2206: unquoted expansion into an array.",
        'list="a b c"',
        "arr=($list)",
        "printf '%s\\n' \"${arr[0]}\"",
        "",
      ].join("\n"),
    );
    const res = await run(["bash", script], { cwd: sb.project });
    expect(res.exitCode).not.toBe(0);
    expect(res.stdout + res.stderr).toContain("SC2206");
  },
  TIMEOUT_MS,
);

test.concurrent.skipIf(NO_SHELLCHECK)(
  "shellcheck.sh: a concern defect is caught by the assembled pass",
  async () => {
    // Concerns are excluded from the standalone pass, so the assembled pass is the
    // ONLY thing that can see a defect in a fragment. A variable assigned in one
    // fragment and consumed by no other is dead across the whole module (SC2034) —
    // exactly the cross-fragment question a per-file lint cannot answer.
    using sb = createSandbox();
    const script = fixtureTree(sb);
    sb.write(
      "plugins/demo/hooks/concerns/00-preamble.sh",
      '#!/usr/bin/env bash\nCOUNT=3\nNEVER_CONSUMED="dead"\n',
    );
    sb.write(
      "plugins/demo/hooks/concerns/10-use.sh",
      "if [ \"$COUNT\" -gt 2 ]; then\n  printf 'many\\n'\nfi\n",
    );
    const res = await run(["bash", script], { cwd: sb.project });
    expect(res.exitCode).not.toBe(0);
    const output = res.stdout + res.stderr;
    expect(output).toContain("SC2034");
    expect(output).toContain("NEVER_CONSUMED");
  },
  TIMEOUT_MS,
);

test.concurrent.skipIf(NO_SHELLCHECK)(
  "shellcheck.sh: reports the missing binary instead of passing silently",
  async () => {
    // A clean environment with a PATH holding only the interpreters the script
    // needs, so `command -v shellcheck` misses while bash/find/mktemp still resolve.
    using sb = createSandbox();
    const script = fixtureTree(sb);
    const stub = sb.path("stub");
    mkdirSync(stub, { recursive: true });
    for (const tool of [
      "bash",
      "find",
      "mktemp",
      "basename",
      "dirname",
      "cat",
      "rm",
      "echo",
      "sort",
    ]) {
      const source = Bun.which(tool);
      if (source !== null) {
        symlinkSync(source, `${stub}/${tool}`);
      }
    }
    const bash = Bun.which("bash");
    expect(bash).not.toBeNull();
    const cleared = Object.fromEntries(Object.keys(process.env).map((key) => [key, undefined]));
    const res = await run([bash ?? "bash", script], {
      cwd: sb.project,
      env: { ...cleared, PATH: stub, HOME: sb.project },
    });
    expect(res.exitCode).toBe(1);
    expect(res.stdout + res.stderr).toContain("shellcheck not on PATH");
  },
  TIMEOUT_MS,
);
