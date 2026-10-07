import { afterAll, expect, test } from "bun:test";
import { mkdirSync, readlinkSync, readdirSync, symlinkSync } from "node:fs";
import { join } from "node:path";
import { ASSET, archive, disposeSigner, signedSums } from "./install-fixture.ts";
import { install, runInstalled, sandboxed, stable } from "./install-runner.ts";

afterAll(disposeSigner);

test.concurrent(
  "signed files of an older release are refused under a newer tag",
  sandboxed(async ({ scratch, serve }) => {
    const dir = scratch();
    const old = await stable("8.0.0");
    expect((await install(serve([[old]]), ["--install-dir", dir])).exitCode).toBe(0);
    const replayed = { tag: "v9.0.0", files: old.files };
    const runs = await Promise.all(
      [[], ["--version", "v9.0.0"]].map((args) =>
        install(serve([[replayed]]), [...args, "--install-dir", dir]),
      ),
    );
    for (const result of runs) {
      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("signature is for another release than v9.0.0");
    }
    expect(runInstalled(dir)).toBe("toolu 8.0.0");
  }),
);

test.concurrent(
  "a Homebrew link or a directory named toolu is never replaced or removed",
  sandboxed(async ({ scratch, serve }) => {
    const prefix = scratch();
    const cellar = join(prefix, "Cellar/toolu/8.0.0/bin");
    mkdirSync(cellar, { recursive: true });
    const dir = join(prefix, "bin");
    mkdirSync(dir);
    symlinkSync("../Cellar/toolu/8.0.0/bin/toolu", join(dir, "toolu"));
    const server = serve([[await stable("9.0.0")]]);
    const runs = await Promise.all(
      [
        ["--install-dir", dir],
        ["--uninstall", "--install-dir", dir],
      ].map((args) => install(server, args)),
    );
    for (const result of runs) {
      expect(result.exitCode).toBe(1);
      expect(result.stderr).toContain("belongs to Homebrew; use brew upgrade toolu");
    }
    expect(readlinkSync(join(dir, "toolu"))).toBe("../Cellar/toolu/8.0.0/bin/toolu");
    expect(server.requests).toEqual([]);
    const other = scratch();
    mkdirSync(join(other, "toolu"));
    const directory = await install(server, ["--install-dir", other]);
    expect(directory.exitCode).toBe(1);
    expect(directory.stderr).toContain("is a directory");
    expect(readdirSync(join(other, "toolu"))).toEqual([]);
  }),
);

test.concurrent(
  "an archive whose toolu is a symlink installs nothing",
  sandboxed(async ({ scratch, serve }) => {
    const tarball = archive("9.0.0", ["toolu", "LICENSE"], true);
    const { sums, minisig } = await signedSums({ [ASSET]: tarball }, "v9.0.0");
    const files = { [ASSET]: tarball, SHA256SUMS: sums, "SHA256SUMS.minisig": minisig };
    const dir = scratch();
    const result = await install(serve([[{ tag: "v9.0.0", files }]]), ["--install-dir", dir]);
    expect(result.exitCode).toBe(1);
    expect(result.stderr).toContain("has a member that is not a regular file");
    expect(readdirSync(dir)).toEqual([]);
  }),
);
