export const wantsLogs = ({ key, token, peer, tab, visible }) =>
  Boolean(key && token && !peer && tab === "logs" && visible);

const MAX_VIEW_CHARS = 1_048_576;

function escapeEnd(value, index) {
  const kind = value[index + 1];
  let end = index + 2;
  if (kind === "[") {
    while (end < value.length && value.charCodeAt(end) >= 32 && value.charCodeAt(end) <= 63)
      end += 1;
    return end + 1;
  }
  if ("]P^_".includes(kind)) {
    while (end < value.length) {
      if (value.charCodeAt(end++) === 7) break;
      if (value.charCodeAt(end - 1) === 27 && value[end] === "\\") return end + 1;
    }
    return end;
  }
  return end + ("()*+-./".includes(kind) ? 1 : 0);
}

function visibleTail(value) {
  const cut = Math.max(0, value.length - MAX_VIEW_CHARS);
  for (let index = 0; index < cut;) {
    if (value.charCodeAt(index) !== 27) {
      index += 1;
      continue;
    }
    const end = escapeEnd(value, index);
    if (end > cut) return value.slice(end);
    index = end;
  }
  return value.slice(cut);
}

export function appendPage(current, page) {
  const reset = page.truncated || current.nextOffset !== page.offset;
  return {
    raw: visibleTail((reset ? "" : current.raw) + page.text),
    nextOffset: page.next_offset,
    state: page.state,
    truncated: page.truncated,
  };
}

export function stripAnsi(value) {
  let visible = "";
  for (let index = 0; index < value.length;) {
    if (value.charCodeAt(index) !== 27) {
      visible += value[index++];
      continue;
    }
    index = escapeEnd(value, index);
  }
  return visible;
}
