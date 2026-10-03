import { expect, test } from "bun:test";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { loadLaunchRecord } from "../launch-state.ts";

test.concurrent("missing launch record starts empty", () => {
  using sb = createSandbox();
  expect(loadLaunchRecord(join(sb.root, "missing.json"))).toEqual({});
});

test.concurrent("valid launch records retain known and extension fields", () => {
  using sb = createSandbox();
  const path = sb.write("issue.json", {
    stage: "running",
    launches: 2,
    lease_token: "lease-1",
    resource_root: "/tmp/resources",
    pane_id: "pane-1",
    worktree: "/tmp/worktree",
    session_id: "session-1",
    session_started_after: 1_797_863_400_000,
    prompt_state: "acknowledged",
    extension: { durable: true },
  });
  expect(loadLaunchRecord(path)).toEqual({
    stage: "running",
    launches: 2,
    lease_token: "lease-1",
    resource_root: "/tmp/resources",
    pane_id: "pane-1",
    worktree: "/tmp/worktree",
    session_id: "session-1",
    session_started_after: 1_797_863_400_000,
    prompt_state: "acknowledged",
    extension: { durable: true },
  });
});

test.concurrent("malformed and non-object launch records fail closed", () => {
  using sb = createSandbox();
  for (const [name, source] of [
    ["malformed", '{"stage":'],
    ["array", "[]"],
    ["null", "null"],
    ["string", '"running"'],
    ["number", "2"],
  ] as const) {
    const path = sb.write(`records/${name}.json`, source);
    expect(() => loadLaunchRecord(path)).toThrow("invalid launch record");
  }
});

test.concurrent("only known current and legacy stages are accepted", () => {
  using sb = createSandbox();
  const allowed = [
    "uncertain",
    "replacing",
    "starting",
    "cleaning",
    "cleanup-incomplete",
    "running",
    "awaiting_merge",
    "merged",
    "abandoned",
  ];
  for (const [index, stage] of allowed.entries()) {
    const path = sb.write(`allowed/${index}.json`, { stage });
    expect(loadLaunchRecord(path).stage).toBe(stage);
  }
  for (const [index, stage] of ["ready", "", null, 3].entries()) {
    const path = sb.write(`invalid/${index}.json`, { stage });
    expect(() => loadLaunchRecord(path)).toThrow("unknown stage");
  }
});

test.concurrent("launch count is a nonnegative safe integer", () => {
  using sb = createSandbox();
  for (const launches of [0, 1, 42]) {
    const path = sb.write(`valid/${launches}.json`, { launches });
    expect(loadLaunchRecord(path).launches).toBe(launches);
  }
  for (const [index, launches] of [-1, 0.5, "1", null, Number.MAX_SAFE_INTEGER + 1].entries()) {
    const path = sb.write(`invalid/${index}.json`, { launches });
    expect(() => loadLaunchRecord(path)).toThrow("launches must be a nonnegative integer");
  }
});

test.concurrent("session capture epoch must be finite and nonnegative", () => {
  using sb = createSandbox();
  const valid = sb.write("valid.json", { session_started_after: 1_797_863_400_000 });
  expect(loadLaunchRecord(valid).session_started_after).toBe(1_797_863_400_000);
  for (const [index, session_started_after] of [-1, "0", null].entries()) {
    const path = sb.write(`invalid/${index}.json`, { session_started_after });
    expect(() => loadLaunchRecord(path)).toThrow("session_started_after must be a finite epoch");
  }
  const infinite = sb.write("invalid/infinite.json", '{"session_started_after":1e400}');
  expect(() => loadLaunchRecord(infinite)).toThrow("session_started_after must be a finite epoch");
});

test.concurrent("durable identity fields require non-empty strings", () => {
  using sb = createSandbox();
  const fields = ["lease_token", "resource_root", "pane_id", "worktree", "session_id"];
  for (const field of fields) {
    for (const [index, value] of [3, false, {}, [], "", "  "].entries()) {
      const path = sb.write(`invalid/${field}-${index}.json`, { [field]: value });
      expect(() => loadLaunchRecord(path)).toThrow(`${field} must be a non-empty string`);
    }
  }
});

test.concurrent("session capture may be pending but acknowledged records require it", () => {
  using sb = createSandbox();
  const pending = sb.write("pending.json", {
    stage: "starting",
    prompt_state: "submitting",
    session_id: null,
  });
  expect(loadLaunchRecord(pending).session_id).toBeNull();

  const acknowledgedNull = sb.write("acknowledged-null.json", {
    stage: "running",
    prompt_state: "acknowledged",
    session_id: null,
  });
  expect(() => loadLaunchRecord(acknowledgedNull)).toThrow(
    "session_id may be null only while awaiting capture",
  );

  const managedMissing = sb.write("acknowledged-missing.json", {
    stage: "running",
    prompt_state: "acknowledged",
    lease_token: "lease-1",
  });
  expect(() => loadLaunchRecord(managedMissing)).toThrow(
    "a managed running launch requires session_id",
  );

  const legacyMissing = sb.write("legacy-running.json", {
    stage: "running",
    prompt_state: "acknowledged",
  });
  expect(loadLaunchRecord(legacyMissing).session_id).toBeUndefined();
});

test.concurrent("cleanup and terminal records preserve lifecycle outcomes without a session", () => {
  using sb = createSandbox();
  for (const stage of ["cleaning", "cleanup-incomplete", "merged", "abandoned"]) {
    const path = sb.write(`${stage}.json`, {
      stage,
      lease_token: "lease-1",
      prompt_state: "acknowledged",
      session_id: null,
      cleanup_error: "owned workload remains",
      cleanup_attempt_at: "2026-10-03T18:00:00Z",
      worktree_removed: false,
      branch_deleted: false,
      wip_ref: "refs/epic-wip/example-1",
      leftover_files: [" M src/example.ts"],
    });
    expect(loadLaunchRecord(path)).toMatchObject({
      stage,
      session_id: null,
      cleanup_error: "owned workload remains",
      worktree_removed: false,
      branch_deleted: false,
      leftover_files: [" M src/example.ts"],
    });
  }
});
