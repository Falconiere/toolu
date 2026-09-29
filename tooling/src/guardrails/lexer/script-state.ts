/**
 * Script-lexer state that persists across physical lines: the lexical state,
 * the brace frames that return template and JSX expressions to their owner,
 * the JSX nesting frames, and a 128-char code-only context window.
 */

export type Frame = { state: string; depth: number };

export type ScriptState = {
  state: string;
  braceDepth: number;
  braces: Frame[];
  jsxDepth: number;
  jsxRoots: Frame[];
  tagKind: string;
  tagLast: string;
  regexClass: boolean;
  context: string;
  parens: string[];
  afterControl: boolean;
  jsxEnabled: boolean;
  /** Whole-file source for multiline generic lookahead, and this line's offset in it. */
  source: string | undefined;
  sourceOffset: number;
};

export function newScriptState(path: string): ScriptState {
  return {
    state: "code",
    braceDepth: 0,
    braces: [],
    jsxDepth: 0,
    jsxRoots: [],
    tagKind: "",
    tagLast: "",
    regexClass: false,
    context: "",
    parens: [],
    afterControl: false,
    jsxEnabled: /\.(tsx|jsx|astro)$/.test(path),
    source: undefined,
    sourceOffset: 0,
  };
}

/** Enter an expression (`${` or `{` in JSX) that returns to `owner` at its closing brace. */
export function pushBrace(st: ScriptState, owner: string): void {
  st.braces.unshift({ state: owner, depth: st.braceDepth });
  st.braceDepth = 1;
  st.state = "code";
}

export function popBrace(st: ScriptState): void {
  const frame = st.braces.shift();
  st.state = frame?.state ?? "";
  st.braceDepth = frame?.depth ?? 0;
}

export function startJsxRoot(st: ScriptState, kind: string): void {
  st.jsxRoots.unshift({ state: "code", depth: st.jsxDepth });
  st.jsxDepth = 0;
  st.state = "jsx_tag";
  st.tagKind = kind;
  st.tagLast = "";
}

export function finishJsxRoot(st: ScriptState): void {
  const frame = st.jsxRoots.shift();
  st.state = frame?.state ?? "";
  st.jsxDepth = frame?.depth ?? 0;
}
