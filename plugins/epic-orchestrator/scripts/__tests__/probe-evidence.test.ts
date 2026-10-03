import { expect, test } from "bun:test";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import {
  DEFAULT_EVIDENCE_PATH,
  PROBE_CAPABILITIES,
  PROBE_HOSTS,
  readProbeEvidence,
  summarizeProbeEvidence,
} from "../probe.ts";

const PROBE = join(import.meta.dir, "..", "probe.ts");

test("committed host evidence preserves every capability gap", () => {
  const evidence = readProbeEvidence();
  expect(Object.keys(evidence.hosts).sort()).toEqual([...PROBE_HOSTS].sort());
  for (const host of PROBE_HOSTS) {
    expect(Object.keys(evidence.hosts[host].capabilities).sort()).toEqual(
      [...PROBE_CAPABILITIES].sort(),
    );
  }
  const summary = summarizeProbeEvidence(evidence);
  expect(summary).not.toContain("codex.cancel_replace=failed");
  expect(summary).toContain("cursor.start=unverified");
  expect(summary).toContain("claude.teardown=unverified");
});

test("--check-evidence validates the record without claiming gaps passed", async () => {
  const proc = Bun.spawn([process.execPath, PROBE, "--check-evidence"], {
    stdout: "pipe",
    stderr: "pipe",
  });
  const [stdout, stderr, exitCode] = await Promise.all([
    new Response(proc.stdout).text(),
    new Response(proc.stderr).text(),
    proc.exited,
  ]);
  expect({ exitCode, stderr }).toEqual({ exitCode: 0, stderr: "" });
  expect(stdout).toContain("probe-evidence: valid schema=1 contract=1");
  expect(stdout).toContain("gaps:");
  expect(stdout).not.toContain("gaps: none");
});

test("--check-evidence rejects a record with an omitted host", async () => {
  const dir = mkdtempSync(join(tmpdir(), "toolu-host-probe-"));
  try {
    const value = JSON.parse(readFileSync(DEFAULT_EVIDENCE_PATH, "utf8")) as {
      hosts: Record<string, unknown>;
    };
    delete value.hosts["cursor"];
    const path = join(dir, "missing-host.json");
    writeFileSync(path, `${JSON.stringify(value)}\n`);
    const proc = Bun.spawn([process.execPath, PROBE, "--check-evidence", path], {
      stdout: "pipe",
      stderr: "pipe",
    });
    const [stdout, stderr, exitCode] = await Promise.all([
      new Response(proc.stdout).text(),
      new Response(proc.stderr).text(),
      proc.exited,
    ]);
    expect(exitCode).toBe(1);
    expect(stdout).toBe("");
    expect(stderr).toContain("evidence.hosts: expected keys claude, codex, cursor, opencode");
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
