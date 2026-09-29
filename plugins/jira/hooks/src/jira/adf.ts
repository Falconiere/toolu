/**
 * A plain-text comment/description body for the API version: a minimal ADF
 * document for v3 (Cloud), the text itself as a JSON string for v2 (Server/DC).
 * Rich ADF (tables, mentions) is out of scope; `jira raw` reaches it.
 */
export function textBody(version: "2" | "3", text: string): unknown {
  if (version === "2") return text;
  return {
    type: "doc",
    version: 1,
    content: [{ type: "paragraph", content: [{ type: "text", text }] }],
  };
}
