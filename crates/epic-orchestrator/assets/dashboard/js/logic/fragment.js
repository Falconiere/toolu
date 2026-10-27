// The URL fragment is an `&`-separated list: `#token=<t>&issue=<key>`.
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
