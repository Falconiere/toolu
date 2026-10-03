/** Separate process competing for a durable agent reservation. */
import { acquireLease } from "../resources.ts";
const [root, epic] = process.argv.slice(2);
if (!root || !epic) throw new Error("root and epic required");
try {
  const lease = await acquireLease(root, {
    type: "agent",
    key: epic,
    stateDir: epic,
    host: "claude",
  });
  process.stdout.write(JSON.stringify(lease));
} catch (error) {
  process.stderr.write(String(error));
  process.exit(75);
}
