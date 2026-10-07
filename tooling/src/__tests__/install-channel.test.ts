import { afterAll, expect, test } from "bun:test";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  readFileSync,
  readdirSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import {
  ASSET,
  archive,
  disposeSigner,
  releaseFiles,
  sha256,
  signedSums,
} from "./install-fixture.ts";
import { REPO, fakeUname, install, runInstalled, sandboxed, stable } from "./install-runner.ts";

const IS_ROOT = process.getuid?.() === 0;

afterAll(disposeSigner);

test.concurrent(
  "a no-flag install writes the verified toolu with mode 0755 (AC-1, AC-9)",
  sandboxed(async ({ scratch, serve }) => {
    const release = await stable("9.0.0");
    const dir = scratch();
    const result = await install(serve([[release]]), ["--install-dir", dir]);
    expect(result.stderr).toBe("");
    expect(result.exitCode).toBe(0);
    expect(result.stdout).toContain("toolu plugins install");
    expect(result.stdout).toContain("toolu doctor");
    expect(statSync(join(dir, "toolu")).mode & 0o777).toBe(0o755);
    expect(runInstalled(dir)).toBe("toolu 9.0.0");
    expect(readdirSync(dir)).toEqual(["toolu"]);
    const unpack = scratch();
    writeFileSync(join(unpack, ASSET), release.files[ASSET] ?? new Uint8Array());
    Bun.spawnSync(["tar", "-xzf", ASSET, "toolu"], { cwd: unpack });
    expect(readFileSync(join(dir, "toolu"))).toEqual(readFileSync(join(unpack, "toolu")));
  }),
);

test.concurrent(
  "the pinned minisign is fetched and checked when TOOLU_MINISIGN is unset",
  sandboxed(async ({ scratch, serve }) => {
    const dir = scratch();
    const server = serve([[await stable("9.0.0")]]);
    const result = await install(server, ["--install-dir", dir], { TOOLU_MINISIGN: "" });
    expect(result.exitCode).toBe(0);
    expect(runInstalled(dir)).toBe("toolu 9.0.0");
  }),
);

test.concurrent(
  "an archive that does not match the signed sums is refused (AC-3)",
  sandboxed(async ({ scratch, serve }) => {
    const dir = scratch();
    expect((await install(serve([[await stable("9.0.0")]]), ["--install-dir", dir])).exitCode).toBe(
      0,
    );
    const files = await releaseFiles("9.1.0");
    const swapped = archive("6.6.6");
    const server = serve([[{ tag: "v9.1.0", files: { ...files, [ASSET]: swapped } }]]);
    const result = await install(server, ["--install-dir", dir]);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain(`expected ${sha256(files[ASSET] ?? new Uint8Array())}`);
    expect(result.stderr).toContain(`got ${sha256(swapped)}`);
    expect(runInstalled(dir)).toBe("toolu 9.0.0");
  }),
);

test.concurrent(
  "--check downloads nothing; --version pins; --uninstall is idempotent (AC-4)",
  sandboxed(async ({ scratch, serve }) => {
    const server = serve([[await stable("9.1.0"), await stable("9.0.0")]]);
    const dir = scratch();
    const check = await install(server, ["--check", "--install-dir", dir]);
    expect(check.exitCode).toBe(0);
    expect(check.stdout).toContain(`install dir: ${dir}`);
    expect(check.stdout).toContain(`/dl/${REPO}/releases/download/<latest>/${ASSET}`);
    expect(server.requests).toEqual([]);
    expect((await install(server, ["--version", "v9.0.0", "--install-dir", dir])).exitCode).toBe(0);
    expect(runInstalled(dir)).toBe("toolu 9.0.0");
    expect(server.requests.some((path) => path.includes("/releases?"))).toBe(false);
    writeFileSync(join(dir, "keep"), "other file");
    expect((await install(undefined, ["--uninstall", "--install-dir", dir])).exitCode).toBe(0);
    expect(readdirSync(dir)).toEqual(["keep"]);
    expect((await install(undefined, ["--uninstall", "--install-dir", dir])).exitCode).toBe(0);
  }),
);

test.concurrent(
  "a missing or rejected signature is refused despite a matching digest (AC-6)",
  sandboxed(async ({ scratch, serve }) => {
    const files = await releaseFiles("9.1.0");
    const unsigned = Object.fromEntries(
      Object.entries(files).filter(([name]) => name !== "SHA256SUMS.minisig"),
    );
    const signature = new TextDecoder().decode(files["SHA256SUMS.minisig"]).split("\n");
    const line = signature[1] ?? "";
    signature[1] = `${line.slice(0, 40)}${line[40] === "A" ? "B" : "A"}${line.slice(41)}`;
    const flipped = {
      ...files,
      "SHA256SUMS.minisig": new TextEncoder().encode(signature.join("\n")),
    };
    const first = await stable("9.0.0");
    await Promise.all(
      [unsigned, flipped].map(async (variant) => {
        const dir = scratch();
        expect((await install(serve([[first]]), ["--install-dir", dir])).exitCode).toBe(0);
        const server = serve([[{ tag: "v9.1.0", files: variant }]]);
        const result = await install(server, ["--version", "v9.1.0", "--install-dir", dir]);
        expect(result.exitCode).toBe(1);
        expect(runInstalled(dir)).toBe("toolu 9.0.0");
      }),
    );
  }),
);

test.concurrent(
  "the newest stable release with assets wins, across pages (AC-7)",
  sandboxed(async ({ scratch, serve }) => {
    const v8 = await stable("8.0.0");
    const rc = { tag: "v9.1.0-rc.1", prerelease: true, files: await releaseFiles("9.1.0") };
    const draft = { tag: "v9.2.0", draft: true, files: await releaseFiles("9.2.0") };
    const empty = { tag: "v9.0.0", files: {} };
    await Promise.all(
      [[[draft, rc, empty, v8]], [[rc], [empty, v8]]].map(async (pages) => {
        const dir = scratch();
        const result = await install(serve(pages), ["--install-dir", dir]);
        expect(result.stderr).toBe("");
        expect(runInstalled(dir)).toBe("toolu 8.0.0");
      }),
    );
    const later = serve([[await stable("9.0.0"), v8]]);
    const dir = scratch();
    expect((await install(later, ["--version", "v8.0.0", "--install-dir", dir])).exitCode).toBe(0);
    expect(runInstalled(dir)).toBe("toolu 8.0.0");
  }),
);

test.concurrent(
  "no stable release with assets installs nothing",
  sandboxed(async ({ scratch, serve }) => {
    const dir = scratch();
    const result = await install(serve([[{ tag: "v9.0.0", files: {} }]]), ["--install-dir", dir]);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("no stable release");
    expect(readdirSync(dir)).toEqual([]);
  }),
);

test.concurrent(
  "an unsupported platform exits 2 before any request (AC-12)",
  sandboxed(async ({ scratch, serve }) => {
    const server = serve([[{ tag: "v9.0.0", files: {} }]]);
    const path = fakeUname(scratch(), '[ "$1" = -s ] && echo FreeBSD || echo arm64');
    const dir = join(scratch(), "target");
    const runs = await Promise.all(
      [["--check"], []].map((args) =>
        install(server, [...args, "--install-dir", dir], { PATH: path }),
      ),
    );
    for (const result of runs) {
      expect(result.exitCode).toBe(2);
      expect(result.stderr).toContain("supported: Darwin or Linux on arm64 or amd64");
    }
    expect(existsSync(dir)).toBe(false);
    expect(server.requests).toEqual([]);
  }),
);

test.skipIf(IS_ROOT)(
  "an unwritable install dir keeps the installed binary (AC-13)",
  sandboxed(async ({ scratch, serve }) => {
    const dir = scratch();
    expect((await install(serve([[await stable("9.0.0")]]), ["--install-dir", dir])).exitCode).toBe(
      0,
    );
    chmodSync(dir, 0o555);
    const result = await install(serve([[await stable("9.1.0")]]), ["--install-dir", dir]);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("--install-dir");
    expect(runInstalled(dir)).toBe("toolu 9.0.0");
  }),
);

test.concurrent(
  "TOOLU_REPO is validated and selects the fork (AC-14)",
  sandboxed(async ({ scratch, serve }) => {
    const bad = serve([[{ tag: "v9.0.0", files: {} }]]);
    const runs = await Promise.all(
      ["not a repo", "../toolu", "a/b/c", "https://x/y"].map((repo) =>
        install(bad, ["--install-dir", scratch()], { TOOLU_REPO: repo }),
      ),
    );
    expect(runs.map((result) => result.exitCode)).toEqual([2, 2, 2, 2]);
    expect(bad.requests).toEqual([]);
    const fork = serve([[await stable("9.0.0")]], "example/toolu");
    const dir = scratch();
    const result = await install(fork, ["--install-dir", dir], { TOOLU_REPO: "example/toolu" });
    expect(result.exitCode).toBe(0);
    expect(fork.requests.filter((path) => path.includes("/example/toolu/")).length).toBe(4);
  }),
);

test.concurrent(
  "an archive without exactly toolu and LICENSE installs nothing (AC-15)",
  sandboxed(async ({ scratch, serve }) => {
    await Promise.all(
      [["toolu"], ["LICENSE", "toolu"]].map(async (members) => {
        const tarball = archive("9.0.0", members);
        const { sums, minisig } = await signedSums({ [ASSET]: tarball }, "v9.0.0");
        const files = { [ASSET]: tarball, SHA256SUMS: sums, "SHA256SUMS.minisig": minisig };
        const dir = scratch();
        const result = await install(serve([[{ tag: "v9.0.0", files }]]), ["--install-dir", dir]);
        expect(result.exitCode).toBe(1);
        expect(result.stderr).toContain("exactly toolu and LICENSE");
        expect(readdirSync(dir)).toEqual([]);
      }),
    );
  }),
);

test.concurrent(
  "usage errors exit 2 and --help needs no platform or network",
  sandboxed(async ({ scratch }) => {
    const bin = scratch();
    const marker = join(bin, "uname-ran");
    const help = await install(undefined, ["--help"], {
      PATH: fakeUname(bin, `touch "${marker}"\nexit 1`),
    });
    expect(help.exitCode).toBe(0);
    expect(help.stdout).toContain("--install-dir <dir>");
    expect(existsSync(marker)).toBe(false);
    const usage = [["--check", "--uninstall"], ["--version", "9.0.0"], ["--version"], ["--bogus"]];
    const runs = await Promise.all(usage.map((args) => install(undefined, args)));
    expect(runs.map((result) => result.exitCode)).toEqual([2, 2, 2, 2]);
  }),
);

test.concurrent(
  "a missing install dir is created",
  sandboxed(async ({ scratch, serve }) => {
    const parent = join(scratch(), "nested");
    mkdirSync(parent);
    const dir = join(parent, "bin");
    expect((await install(serve([[await stable("9.0.0")]]), ["--install-dir", dir])).exitCode).toBe(
      0,
    );
    expect(runInstalled(dir)).toBe("toolu 9.0.0");
  }),
);
