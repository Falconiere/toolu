/**
 * Line counters against the unmodified `detect.sh` (#254 AC-4): every tracked
 * `*.ts`, `*.rs`, `*.py` and `*.sh` file in this repository, the snippets its
 * bats suite counts, the boundaries awk defines (CRLF, no final newline,
 * empty, missing, a directory, latin1 bytes), a file over 1 MiB, and memory
 * that stays flat while a 256 MiB file is counted.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { closeSync, mkdirSync, openSync, readFileSync, writeFileSync, writeSync } from "node:fs";
import { join, resolve } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import { z } from "zod";
import { countCodeLines, countPythonCodeLines, hasUnterminatedBlock } from "../detect-lines.ts";
import { eachLine } from "../detect-read.ts";
import { bashDetect, detectEnv } from "./detect-bash.ts";

const REPO = resolve(import.meta.dir, "../../../../..");
const COUNT_BODY = `for f in "$@"; do
  if has_unterminated_block "$f"; then u=1; else u=0; fi
  printf '%s,%s,%s\\037' "$(count_code_lines "$f")" "$(count_python_code_lines "$f")" "$u"
done`;

/** `count_code_lines,count_python_code_lines,has_unterminated_block` per file, as bash prints them. */
async function bashCounts(files: readonly string[], home: string): Promise<string[]> {
  const out = await bashDetect(COUNT_BODY, files, REPO, detectEnv(home));
  return out.split("\x1f").slice(0, -1);
}

function tsCounts(files: readonly string[]): string[] {
  return files.map((f) =>
    [countCodeLines(f) ?? "", countPythonCodeLines(f) ?? "", hasUnterminatedBlock(f) ? 1 : 0].join(
      ",",
    ),
  );
}

function trackedSources(): string[] {
  const res = spawnSync("git", ["-C", REPO, "ls-files", "-z", "*.ts", "*.rs", "*.py", "*.sh"], {
    encoding: "utf8",
  });
  return res.stdout
    .split("\0")
    .filter((f) => f !== "")
    .map((f) => join(REPO, f));
}

const SOURCES = trackedSources();
const CHUNK = 150;
const BATCHES = Array.from({ length: Math.ceil(SOURCES.length / CHUNK) }, (_, i) =>
  SOURCES.slice(i * CHUNK, (i + 1) * CHUNK),
);

test("the repository holds real sources of every language counted", () => {
  for (const ext of [".ts", ".rs", ".py", ".sh"]) {
    expect(SOURCES.some((f) => f.endsWith(ext))).toBe(true);
  }
  expect(SOURCES.length).toBeGreaterThan(500);
});

test.concurrent.each(BATCHES.map((files, i) => [i, files] as const))(
  "tracked sources batch %d: every count equals bash",
  async (_i, files) => {
    using sb = createSandbox();
    expect(tsCounts(files)).toEqual(await bashCounts(files, sb.home));
  },
  120_000,
);

const SNIPPETS: Readonly<Record<string, string>> = {
  "blanks-and-line-comments.ts": "const a = 1;\n\n// comment\n  // indented\nconst b = 2;\n",
  "multi-line-block.ts": "/*\n * doc\n */\nconst a = 1;\n",
  "trailing-comment.ts": "const a = 1; // trailing\n",
  "inline-and-rust-doc.rs": "let x = /* inline */ 1;\n/// doc\n//! inner\nfn main() {}\n",
  "unterminated.ts": 'const s = "/* x";\nconst a = 1;\nconst b = 2;\n',
  "two-inline-blocks.ts": "  /* a */ b /* c */ \n/* only */\n",
  "close-before-open.ts": "a */ b /* c\nd\n*/ e\n",
  "nested-open.ts": "/* /* */ x */\n",
  "slash-star-slash.ts": "/*/ still comment\n*/ x\n",
  "crlf.ts": "const a = 1;\r\n\r\n// c\r\n",
  "no-final-newline.ts": "const a = 1;\nconst b = 2;",
  "empty.ts": "",
  "only-newlines.ts": "\n\n\n",
  "tabs-and-spaces.py": "\t\n  # comment\n\tx = 1  # trailing\n'''doc'''\n#!shebang\n",
  "docstring.py": 'def f():\n    """Doc.\n\n    # not a comment line\n    """\n    return 1\n',
  "latin1-bytes.ts": "const s = '\xe9\xff'; // \xe9\n/* \xe9 */\n",
};

test("edge snippets, a missing path and a directory equal bash", async () => {
  using sb = createSandbox();
  const files = Object.entries(SNIPPETS).map(([name, body]) => {
    const path = sb.path(name);
    writeFileSync(path, Buffer.from(body, "latin1"));
    return path;
  });
  mkdirSync(sb.path("a-dir"));
  files.push(sb.path("missing.ts"), sb.path("a-dir"));
  expect(tsCounts(files)).toEqual(await bashCounts(files, sb.home));
});

test("the bats snippets give the counts bats asserts", () => {
  using sb = createSandbox();
  const write = (name: string) => {
    const path = sb.path(name);
    writeFileSync(path, SNIPPETS[name] ?? "");
    return path;
  };
  expect(countCodeLines(write("blanks-and-line-comments.ts"))).toBe(2);
  expect(countCodeLines(write("multi-line-block.ts"))).toBe(1);
  expect(countCodeLines(write("trailing-comment.ts"))).toBe(1);
  expect(countCodeLines(write("inline-and-rust-doc.rs"))).toBe(2);
  expect(countCodeLines(write("unterminated.ts"))).toBe(3);
  expect(hasUnterminatedBlock(write("unterminated.ts"))).toBe(true);
  expect(countPythonCodeLines(write("docstring.py"))).toBe(4);
  expect(countCodeLines(sb.path("missing.ts"))).toBeUndefined();
});

test("NUL bytes are content (bash depends on the host's awk and grep there)", () => {
  // macOS awk ends a record at NUL and grep -o prints one "Binary file matches"
  // line for a file holding one, so bash answers 1,2,0 on macOS and differently
  // on Linux. The port counts what is written. Named deviation, docs/detect.md.
  using sb = createSandbox();
  const path = sb.path("nul-bytes.ts");
  writeFileSync(path, "/* /* */\0\nd\n");
  expect(tsCounts([path])).toEqual(["2,2,1"]);
});

test("a file over 1 MiB counts the same as bash", async () => {
  using sb = createSandbox();
  const big = sb.path("big.ts");
  const parts: Buffer[] = [];
  let size = 0;
  for (const file of SOURCES.filter((f) => f.endsWith(".ts"))) {
    const body = readFileSync(file);
    parts.push(body);
    size += body.length;
    if (size > 3 << 20) break;
  }
  writeFileSync(big, Buffer.concat(parts));
  expect(size).toBeGreaterThan(1 << 20);
  expect(tsCounts([big])).toEqual(await bashCounts([big], sb.home));
}, 60_000);

/** A file of `mib` MiB made of one real source file repeated. */
function repeatedSource(path: string, mib: number): void {
  const unit = readFileSync(join(REPO, "packages/toolu-core/src/shell/shell-walk.ts"));
  const block = Buffer.concat(
    Array.from({ length: Math.ceil((1 << 20) / unit.length) }, () => unit),
  );
  const fd = openSync(path, "w");
  for (let written = 0; written < mib << 20; written += block.length) writeSync(fd, block);
  closeSync(fd);
}

/** RSS growth of a fresh bun process counting `file`, after one warm-up count. */
async function countingGrowth(probe: string, file: string, home: string): Promise<number> {
  const res = await run([process.execPath, probe, file], {
    env: { HOME: home },
    timeoutMs: 120_000,
  });
  expect(res.exitCode).toBe(0);
  const { lines, grew } = z
    .object({ lines: z.number(), grew: z.number() })
    .parse(JSON.parse(res.stdout));
  expect(lines).toBeGreaterThan(100_000);
  return grew;
}

test("counting a 256 MiB file keeps memory flat", async () => {
  using sb = createSandbox();
  const probe = sb.path("probe.ts");
  writeFileSync(
    probe,
    `import { countCodeLines } from ${JSON.stringify(join(import.meta.dir, "../detect-lines.ts"))};
countCodeLines(process.argv[1] ?? "");
const before = process.memoryUsage().rss;
const lines = countCodeLines(process.argv[2] ?? "");
console.log(JSON.stringify({ lines, grew: process.memoryUsage().rss - before }));
`,
  );
  repeatedSource(sb.path("small.ts"), 16);
  repeatedSource(sb.path("huge.ts"), 256);
  const small = await countingGrowth(probe, sb.path("small.ts"), sb.home);
  const huge = await countingGrowth(probe, sb.path("huge.ts"), sb.home);
  // Heap slack is a constant; what must not happen is growth with the file.
  expect(huge - small).toBeLessThan(16 << 20);
  expect(huge).toBeLessThan(64 << 20);
}, 240_000);

test("a throwing visitor propagates instead of reading as unreadable", () => {
  using sb = createSandbox();
  const path = sb.path("a.ts");
  writeFileSync(path, "a\nb\n");
  expect(() =>
    eachLine(path, () => {
      throw new Error("visitor bug");
    }),
  ).toThrow("visitor bug");
  expect(eachLine(sb.path("missing.ts"), () => undefined)).toBe("unreadable");
  expect(eachLine(sb.root, () => undefined)).toBe("done");
});
