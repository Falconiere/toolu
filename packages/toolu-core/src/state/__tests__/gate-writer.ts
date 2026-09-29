/**
 * One concurrent gate writer for gate-file-concurrency.test.ts (#255): a real
 * Bun process that records `/w/<id>/0..<count-1>` and then clears the even
 * ones, all through the public TypeScript API. `strict` fails the process when
 * a clear finds nothing to clear; `lenient` allows it, for races with unlocked
 * bash writers, which can drop an entry.
 *
 *   bun gate-writer.ts <gate-file> <id> <count> <strict|lenient>
 */
import { clearGateFile, recordGateFailure } from "../gate-file.ts";

const [gate, id, count, mode] = process.argv.slice(2);
if (gate === undefined || id === undefined || count === undefined || mode === undefined) {
  console.error("usage: gate-writer.ts <gate-file> <id> <count> <strict|lenient>");
  process.exit(2);
}
const total = Number(count);
for (let n = 0; n < total; n += 1) {
  recordGateFailure(
    gate,
    `/w/${id}/${String(n)}`,
    `writer-${id}`,
    `reason ${id}`,
    `${id}/${String(n)}\n`,
  );
}
for (let n = 0; n < total; n += 2) {
  const outcome = clearGateFile(gate, `/w/${id}/${String(n)}`, `writer-${id}`);
  if (outcome !== "cleared" && mode === "strict") {
    console.error(`writer ${id}: clear of ${String(n)} was ${outcome}`);
    process.exit(1);
  }
}
