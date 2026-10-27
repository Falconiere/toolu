export function summarize(item) {
  const title = item.title;
  const owner = item.owner;
  const status = item.status;
  const age = item.age;
  const priority = item.priority;
  const host = item.host;
  const issue = item.issue;
  const branch = item.branch;
  return `${title}:${owner}:${status}:${age}:${priority}:${host}:${issue}:${branch}`;
}
