/** Platform memory sampling over real vm_stat and /proc/meminfo output. */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { freemem, totalmem } from "node:os";
import {
  advancePressure,
  darwinAvailableBytes,
  sampleResources,
  vmStatAvailableBytes,
} from "../pressure.ts";

// Captured from a 16 GiB Mac whose `memory_pressure` reported 67% free while
// os.freemem() / os.totalmem() read 0.038.
const VM_STAT = `Mach Virtual Memory Statistics: (page size of 16384 bytes)
Pages free:                                    79141.
Pages active:                                 306165.
Pages inactive:                               292169.
Pages speculative:                             13860.
Pages throttled:                                   0.
Pages wired down:                             119215.
Pages purgeable:                                 175.
"Translation faults":                     5354572810.
Pages copy-on-write:                       730111564.
Pages zero filled:                        1592063278.
Pages reactivated:                           8342528.
Pages purged:                                1107925.
File-backed pages:                            275590.
Anonymous pages:                              336604.
Pages stored in compressor:                   373938.
Pages occupied by compressor:                 204575.
Decompressions:                              4430568.
Compressions:                                7452741.
Pageins:                                    80219212.
Pageouts:                                     133226.
Swapins:                                       23795.
Swapouts:                                     141164.
`;
const MAC_TOTAL = 17_179_869_184;

test.concurrent("vm_stat counts free, inactive and speculative pages as available", () => {
  const available = vmStatAvailableBytes(VM_STAT);
  expect(available).toBe((79_141 + 292_169 + 13_860) * 16_384);
  const sample = { at: 0, cpus: 10, load: 2, availableBytes: available ?? 0, steal: null };
  let pressure = advancePressure(undefined, { ...sample, totalBytes: MAC_TOTAL });
  pressure = advancePressure(pressure, { ...sample, totalBytes: MAC_TOTAL, at: 60_000 });
  expect(pressure.held).toBe(false);
  expect(pressure.badSince).toBeNull();
  expect(pressure.goodSince).toBe(0);
});

test.concurrent("vm_stat output missing a counted line is not parsed", () => {
  expect(vmStatAvailableBytes(VM_STAT.replace(/^Pages speculative:.*\n/m, ""))).toBeNull();
  expect(vmStatAvailableBytes(VM_STAT.replace(/\(page size of \d+ bytes\)/, ""))).toBeNull();
  expect(vmStatAvailableBytes("")).toBeNull();
});

test.concurrent("a vm_stat command that cannot run or parse yields no sample", () => {
  expect(darwinAvailableBytes("/nonexistent/vm_stat")).toBeNull();
  expect(darwinAvailableBytes("/usr/bin/false")).toBeNull();
  expect(darwinAvailableBytes("/bin/echo")).toBeNull();
});

test.if(process.platform === "darwin")("macOS samples reclaimable memory from vm_stat", () => {
  const sample = sampleResources();
  const live = vmStatAvailableBytes(spawnSync("/usr/bin/vm_stat", { encoding: "utf8" }).stdout);
  if (live === null) throw new Error("live vm_stat output did not parse");
  expect(Math.abs(sample.availableBytes - live)).toBeLessThan(totalmem() * 0.05);
  expect(sample.availableBytes).toBeGreaterThanOrEqual(freemem());
  expect(sample.availableBytes).toBeLessThanOrEqual(sample.totalBytes);
});

test.if(process.platform === "linux")("Linux samples MemAvailable from /proc/meminfo", () => {
  const sample = sampleResources();
  const kb = /^MemAvailable:\s+(\d+) kB$/m.exec(readFileSync("/proc/meminfo", "utf8"))?.[1];
  if (kb === undefined) throw new Error("/proc/meminfo has no MemAvailable");
  expect(Math.abs(sample.availableBytes - Number(kb) * 1024)).toBeLessThan(totalmem() * 0.05);
});
