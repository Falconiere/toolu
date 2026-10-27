export const $ = (id) => document.getElementById(id);

export const element = (tag, className, value) => {
  const node = document.createElement(tag);
  if (className) node.className = className;
  if (value !== undefined) node.textContent = String(value);
  return node;
};

export const text = (id, value) => {
  $(id).textContent = value;
};

export function mark(state) {
  const [glyph, ...words] = state.label.split(" ");
  const badge = element("span", `state state-${state.kind}`);
  badge.append(element("span", "glyph", glyph), element("span", "state-word", words.join(" ")));
  return badge;
}
