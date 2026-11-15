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

// True when `data` differs from what `target` was last built from; a rebuild that would
// change nothing is skipped, so a press that starts on a row still ends on it.
export function changed(target, data) {
  const signature = JSON.stringify(data) ?? "";
  if (target.dataset.signature === signature) return false;
  target.dataset.signature = signature;
  return true;
}

// Rebuild inside `target`, then give keyboard focus back to the control `id` names.
export function redraw(target, id, draw) {
  const focused = target.contains(document.activeElement) ? id(document.activeElement) : null;
  draw();
  if (!focused) return;
  [...target.querySelectorAll("button, summary, a")]
    .find((node) => id(node) === focused)
    ?.focus({ preventScroll: true });
}

// Show `link` with `href` and `label`, or hide it when there is no safe URL.
export function showLink(link, href, label) {
  link.hidden = !href;
  if (!href) return;
  link.href = href;
  link.textContent = label;
}
