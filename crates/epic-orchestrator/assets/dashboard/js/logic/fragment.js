// The URL fragment is an `&`-separated list: `#token=<t>&issue=<key>&epic=<key>&view=<name>`.
// `+` stays a plus: `toolu epic token url` prints the token unencoded, and base64 tokens contain `+`.
const params = (hash) => new URLSearchParams((hash ?? "").replace(/^#/, "").replaceAll("+", "%2B"));
const format = (list) => {
  const text = list.toString();
  return text ? `#${text}` : "";
};

export function takeToken(hash) {
  const list = params(hash);
  const token = list.get("token") || null;
  list.delete("token");
  return { token, hash: format(list) };
}

export const fragmentIssue = (hash) => params(hash).get("issue") || null;

export function withIssue(hash, key) {
  const list = params(hash);
  if (key) list.set("issue", key);
  else list.delete("issue");
  return format(list);
}

const PANELS = new Set(["hosts", "journal"]);

// The selected view: an epic wins over a panel, and anything else is the overview.
export function fragmentView(hash) {
  const list = params(hash);
  const key = list.get("epic");
  if (key) return { kind: "epic", key };
  const panel = list.get("view");
  return { kind: PANELS.has(panel) ? panel : "overview" };
}

export function withView(hash, view) {
  const list = params(hash);
  list.delete("epic");
  list.delete("view");
  if (view.kind === "epic") list.set("epic", view.key);
  else if (view.kind !== "overview") list.set("view", view.kind);
  return format(list);
}
