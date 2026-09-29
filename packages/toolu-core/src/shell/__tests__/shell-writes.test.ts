/**
 * Write targets (#284): every output redirection form with or without spaces,
 * redirects on compound commands, and the file operands of commands that write
 * in place or copy. A descriptor duplication is not a file.
 */
import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { analyzeShell } from "../shell-parse.ts";
import { writeTargets } from "../shell-writes.ts";

const paths = (source: string) => writeTargets(analyzeShell(source)).map((t) => t.path);
const vias = (source: string) => writeTargets(analyzeShell(source)).map((t) => [t.via, t.path]);

test.concurrent("every output redirection writes its target, spaced or not", () => {
  for (const source of [
    "echo x > .env",
    "echo x >.env",
    "echo SECRET=1>.env",
    "printf k=v 1>.env",
    "echo x >> .env",
    "echo x &>.env",
    "echo x &>>.env",
    "echo x >| .env",
    "echo x >&.env",
    "exec 3>.env",
    "exec 3<>.env",
    "{ echo x; } >.env",
    "(echo x) > .env",
  ]) {
    expect([source, paths(source).filter((p) => p !== null)]).toEqual([source, [".env"]]);
  }
});

test.concurrent("descriptor duplication and input redirection write nothing", () => {
  for (const source of [
    "cmd 2>&1",
    "cmd 2>&-",
    "cmd >&2",
    "cat < .env",
    "cat <<< x",
    "cat <<EOF\nx > .env\nEOF",
  ]) {
    expect([source, paths(source)]).toEqual([source, []]);
  }
});

test.concurrent("a dynamic redirect target is reported as unknown, not dropped", () => {
  expect(writeTargets(analyzeShell('echo x > "$OUT"')).map((t) => [t.path, t.text])).toEqual([
    [null, '"$OUT"'],
  ]);
  expect(paths("cmd > $(echo .env)")).toEqual([null]);
});

test.concurrent("a compound redirect has no command; a simple one names its command", () => {
  const [compound] = writeTargets(analyzeShell("{ echo x; } >.env"));
  expect(compound?.command).toBeNull();
  const [simple] = writeTargets(analyzeShell("echo x >.env"));
  expect(simple?.command?.argv).toEqual(["echo", "x"]);
});

test.concurrent("tee writes every operand", () => {
  expect(vias("echo hi | tee -a .env notes.txt")).toEqual([
    ["tee", ".env"],
    ["tee", "notes.txt"],
  ]);
});

test.concurrent("sed and perl write their files only in place, never their script", () => {
  expect(paths("sed -i 's/a/b/' .env")).toEqual([".env"]);
  expect(paths("sed -i '' 's/a/b/' .env")).toEqual([".env"]);
  expect(paths("sed -i.bak -e s/a/b/ .env other")).toEqual([".env", "other"]);
  expect(paths("sed --in-place=.bak 's/a/b/' .env")).toEqual([".env"]);
  expect(paths("sed -Ei 's/a/b/' .env")).toEqual([".env"]);
  expect(paths("sed 's/a/b/' .env")).toEqual([]);
  expect(paths("perl -i -pe 's/a/b/' .env")).toEqual([".env"]);
  expect(paths("perl -pi -e 's/a/b/' .env")).toEqual([".env"]);
  expect(paths("perl -ne 'print' .env")).toEqual([]);
});

test.concurrent("cp, mv and install write their destination", () => {
  expect(paths("cp src/.env apps/api/.env 2>&1")).toEqual(["apps/api/.env"]);
  expect(paths("mv source.txt .env")).toEqual([".env"]);
  expect(paths("install -m 644 source.txt .env")).toEqual([".env"]);
  expect(paths("cp -t apps/api src/.env")).toEqual(["apps/api/.env"]);
  expect(paths("cp --target-directory=apps/api/ src/.env b")).toEqual([
    "apps/api/.env",
    "apps/api/b",
  ]);
  expect(paths("cp a/.env b/.npmrc dest")).toEqual(["dest", "dest/.env", "dest/.npmrc"]);
  expect(paths("cp .env dest/")).toEqual(["dest/", "dest/.env"]);
  expect(paths("install -d .env conf")).toEqual([".env", "conf"]);
  expect(paths("cp onlyone")).toEqual([]);
});

test.concurrent("dd writes of=, python writes every open() in a write mode", () => {
  expect(paths("dd if=/dev/zero of=.env bs=1")).toEqual([".env"]);
  expect(paths(`python3 -c "open('ok.txt','w'); open('.env','a+')"`)).toEqual(["ok.txt", ".env"]);
  expect(paths(`python3 -c "open('.env','r+').write(y)"`)).toEqual([".env"]);
  expect(paths(`python -c "print(open('.env','rb').read())"`)).toEqual([]);
  expect(paths('python3.12 -c \'open(".env", "x")\'')).toEqual([".env"]);
  expect(paths("python3 script.py")).toEqual([]);
});

test.concurrent("a write inside bash -c, eval or a substitution is found", () => {
  expect(paths("bash -c 'echo x > .env'")).toEqual([".env"]);
  expect(paths('eval "cp a .env"')).toEqual([".env"]);
  expect(paths('echo "$(echo hi > .env)"')).toEqual([".env"]);
  expect(paths("cat <<EOF\n$(echo hi > .env)\nEOF")).toEqual([".env"]);
  expect(paths("cat <<'EOF'\n$(echo hi > .env)\nEOF")).toEqual([]);
});

/**
 * bash globs an unquoted redirect target or argument and writes the one existing
 * file it matches (checked with neutral names under bash 5.3 and /bin/bash 3.2:
 * `echo pwned > targ[e]t.txt` overwrote target.txt). So `> .en[v]` writes .env,
 * and the target is reported as a pattern that matches the protected name.
 */
const PROTECTED = readFileSync(
  join(import.meta.dir, "../../../../../plugins/toolu/settings/protected-files.txt"),
  "utf8",
)
  .split("\n")
  .filter((line) => line.trim() !== "" && !line.startsWith("#"));

test.concurrent("an unquoted pathname pattern is reported as a pattern matching the protected .env", () => {
  expect(PROTECTED).toContain(".env");
  for (const source of [
    "echo x > .en[v]",
    "echo x >.e?v",
    "printf k 1>.en*",
    "echo x &>> .[e]nv",
    "cp src .en[v]",
    "mv src .e?v",
    "echo x | tee .en[v]",
    "sed -i s/a/b/ .en[v]",
    "{ echo x; } > .e?v",
  ]) {
    const [target] = writeTargets(analyzeShell(source));
    expect([source, target?.path, new Bun.Glob(target?.pattern ?? "").match(".env")]).toEqual([
      source,
      null,
      true,
    ]);
  }
});

test.concurrent("a pattern inside a target directory keeps both parts", () => {
  const [target] = writeTargets(analyzeShell("cp -t apps/api src/.en[v]"));
  expect([target?.path, target?.pattern]).toEqual([null, "apps/api/.en[v]"]);
  expect(new Bun.Glob(target?.pattern ?? "").match("apps/api/.env")).toBe(true);
});

test.concurrent("a quoted or escaped pattern character names the file literally", () => {
  for (const source of [
    "echo x > '.en[v]'",
    'echo x > ".e?v"',
    "echo x > .en\\[v\\]",
    "echo x > .e\\?v",
  ]) {
    const [target] = writeTargets(analyzeShell(source));
    expect([source, target?.pattern]).toEqual([source, null]);
    expect(target?.path).toMatch(/^\.e/);
  }
  expect(paths("dd if=x of=.en[v]")).toEqual([".en[v]"]);
});
