/**
 * How the npm wrapper finds the native binary (#438), over real files and the
 * built `toolu`, with the fixed install directories passed in so the machine's
 * own installs never decide a case.
 */
import { describe, expect, test } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { requiredBuiltTooluBinary } from "@toolu/conformance/harness/entry-command";
import { candidates, catalogPath, findNative, isWrapper, nativeWords } from "../resolve";

const WRAPPER = resolve(import.meta.dir, "../cli.ts");

interface Sandbox {
  readonly root: string;
  readonly bin: string;
  [Symbol.dispose](): void;
}

function sandbox(): Sandbox {
  const root = realpathSync(mkdtempSync(join(tmpdir(), "toolu-plugins-resolve-")));
  const bin = join(root, "bin");
  mkdirSync(bin);
  return { root, bin, [Symbol.dispose]: () => rmSync(root, { recursive: true, force: true }) };
}

function script(path: string, body: string): string {
  writeFileSync(path, `#!/bin/sh\n${body}\n`);
  chmodSync(path, 0o755);
  return path;
}

describe("candidates", () => {
  test("a non-empty TOOLU_BIN is the only candidate", () => {
    expect(candidates({ TOOLU_BIN: "/x/toolu", PATH: "/a:/b", HOME: "/h" }, ["/fixed"])).toEqual(["/x/toolu"]);
  });

  test("otherwise the fixed directories, ~/.local/bin, then PATH, in order", () => {
    expect(candidates({ TOOLU_BIN: "", PATH: "/a::/b", HOME: "/h" }, ["/fixed"])).toEqual([
      "/fixed/toolu",
      "/h/.local/bin/toolu",
      "/a/toolu",
      "/b/toolu",
    ]);
  });
});

describe("findNative", () => {
  test("the wrapper on PATH as toolu, and a legacy script, are skipped: missing", () => {
    using sb = sandbox();
    symlinkSync(WRAPPER, join(sb.bin, "toolu"));
    const legacy = join(sb.root, "legacy");
    mkdirSync(legacy);
    script(join(legacy, "toolu"), 'echo "toolu: unknown flag: $1" >&2; exit 2');
    const found = findNative({ PATH: `${sb.bin}:${legacy}` }, WRAPPER, []);
    expect(found).toEqual({ kind: "missing", searched: [join(sb.bin, "toolu"), join(legacy, "toolu")] });
  });

  test("a native without plugins is old, and a later good one on PATH wins", () => {
    using sb = sandbox();
    const oldDir = join(sb.root, "old");
    mkdirSync(oldDir);
    script(join(oldDir, "toolu"), '[ "$1" = "--hook-protocol" ] && { echo 2; exit 0; }; exit 64');
    expect(findNative({ PATH: oldDir }, WRAPPER, [])).toEqual({ kind: "old", path: join(oldDir, "toolu") });
    // Exit 0 with no usage text is not `plugins install` either.
    script(join(oldDir, "toolu"), '[ "$1" = "--hook-protocol" ] && echo 2; exit 0');
    expect(findNative({ PATH: oldDir }, WRAPPER, [])).toEqual({ kind: "old", path: join(oldDir, "toolu") });
    symlinkSync(requiredBuiltTooluBinary(), join(sb.bin, "toolu"));
    const found = findNative({ PATH: `${oldDir}:${sb.bin}` }, WRAPPER, []);
    expect(found).toEqual({ kind: "found", path: realpathSync(requiredBuiltTooluBinary()) });
  });

  test("each real path is probed once, however many names lead to it", () => {
    using sb = sandbox();
    const log = join(sb.root, "probes.log");
    const native = script(
      join(sb.root, "native"),
      `echo "$*" >> ${log}\n[ "$1" = "--hook-protocol" ] && echo 2\n[ "$1" = plugins ] && echo "Usage: toolu plugins install"\nexit 0`,
    );
    const other = join(sb.root, "other");
    mkdirSync(other);
    symlinkSync(native, join(sb.bin, "toolu"));
    symlinkSync(native, join(other, "toolu"));
    const found = findNative({ PATH: `${sb.bin}:${other}` }, WRAPPER, []);
    expect(found).toEqual({ kind: "found", path: native });
    expect(readFileSync(log, "utf8")).toBe("--hook-protocol\nplugins install --help\n");
  });

  test("a bad TOOLU_BIN is not passed over for a good binary elsewhere", () => {
    using sb = sandbox();
    symlinkSync(requiredBuiltTooluBinary(), join(sb.bin, "toolu"));
    const found = findNative({ TOOLU_BIN: join(sb.root, "typo"), PATH: sb.bin }, WRAPPER, []);
    expect(found).toEqual({ kind: "missing", searched: [join(sb.root, "typo")] });
  });

  test("a copy inside the wrapper's package or a global @toolu/plugins install is the wrapper", () => {
    const self = "/opt/pkg/dist/cli.js";
    expect(isWrapper("/opt/pkg/dist/cli.js", self)).toBe(true);
    expect(isWrapper("/opt/pkg/bin/toolu", self)).toBe(true);
    expect(isWrapper("/usr/lib/node_modules/@toolu/plugins/bin/toolu", self)).toBe(true);
    expect(isWrapper("/opt/other/toolu", self)).toBe(false);
    // A copy of the bundle outside a package does not claim its parent directory.
    expect(isWrapper("/home/me/.local/bin/toolu", "/home/me/bin/toolu")).toBe(false);
  });
});

describe("the words handed to toolu", () => {
  test("--catalog goes after the first known verb, unless the user gave one", () => {
    expect(nativeWords(["install", "jev"], "/c.json")).toEqual(["plugins", "install", "--catalog", "/c.json", "jev"]);
    expect(nativeWords(["--host", "claude", "list"], "/c.json")).toEqual([
      "plugins",
      "--host",
      "claude",
      "list",
      "--catalog",
      "/c.json",
    ]);
    expect(nativeWords(["install", "--catalog", "/mine.json"], "/c.json")).toEqual([
      "plugins",
      "install",
      "--catalog",
      "/mine.json",
    ]);
    expect(nativeWords(["install", "--catalog=/mine.json"], "/c.json")).toEqual(["plugins", "install", "--catalog=/mine.json"]);
    expect(nativeWords([], "/c.json")).toEqual(["plugins"]);
    expect(nativeWords(["--help"], "/c.json")).toEqual(["plugins", "--help"]);
    expect(nativeWords(["install"], undefined)).toEqual(["plugins", "install"]);
  });

  test("the catalog is the repository's from source and the shipped copy from a bundle", () => {
    expect(catalogPath(WRAPPER)).toBe(resolve(import.meta.dir, "../../../../.claude-plugin/marketplace.json"));
    expect(catalogPath("/pkg/dist/cli.js")).toBe("/pkg/assets/marketplace.json");
  });
});
