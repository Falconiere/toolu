/** Read a Jira issue for an epic worker without requiring the jira plugin. */
import { formatJson } from "@toolu/core/rest";
import { JiraClient } from "./trackers/jira-client.ts";
import { parseJiraRef } from "./trackers/jira.ts";

async function main(args: string[]): Promise<void> {
  const [action, rawKey, extra] = args;
  const key = rawKey ? parseJiraRef(rawKey) : null;
  if (action !== "get" || !key || extra !== undefined) {
    throw new Error("Usage: bun --no-env-file scripts/jira-issue.ts get <KEY>");
  }
  const client = new JiraClient();
  process.stdout.write(formatJson(await client.rawIssue(key)));
}

if (import.meta.main) {
  main(process.argv.slice(2)).catch((error: unknown) => {
    process.stderr.write(`jira issue: ${error instanceof Error ? error.message : String(error)}\n`);
    process.exitCode = 1;
  });
}
