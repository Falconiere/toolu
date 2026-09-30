/**
 * The inline ast-grep rules (#267), verbatim from 60-error-handling.sh and
 * 70-no-mocks.sh. Severity is `warning` on purpose: `scan` exits non-zero on an
 * error-level match, which would be indistinguishable from a crashed tool.
 */

/** Panic-on-error rules for production code. */
export const ERROR_RULES =
  "id: unwrap\nlanguage: rust\nseverity: warning\nmessage: unwrap on Result/Option\nrule:\n  pattern: '$E.unwrap()'\n---\nid: expect\nlanguage: rust\nseverity: warning\nmessage: expect on Result/Option\nrule:\n  pattern: '$E.expect($M)'\n---\nid: panic\nlanguage: rust\nseverity: warning\nmessage: panic!\nrule:\n  pattern: 'panic!($$$)'\n---\nid: todo\nlanguage: rust\nseverity: warning\nmessage: todo!\nrule:\n  pattern: 'todo!($$$)'\n---\nid: unimplemented\nlanguage: rust\nseverity: warning\nmessage: unimplemented!\nrule:\n  pattern: 'unimplemented!($$$)'\n---\nid: unreachable\nlanguage: rust\nseverity: warning\nmessage: unreachable!\nrule:\n  pattern: 'unreachable!($$$)'";

/** Mock definitions in src/. */
export const MOCK_SRC_RULES =
  "id: automock-attr\nlanguage: rust\nseverity: warning\nmessage: '#[automock] mock definition'\nrule:\n  pattern: '#[automock]'\n---\nid: automock-cfg-attr\nlanguage: rust\nseverity: warning\nmessage: cfg_attr(..., automock) mock definition\nrule:\n  pattern: '#[cfg_attr($$$, automock)]'\n---\nid: mock-macro\nlanguage: rust\nseverity: warning\nmessage: 'mock! { ... } mock definition'\nrule:\n  any:\n    - pattern: 'mock! { $$$ }'\n    - pattern: 'mock!($$$)'\n    - pattern: 'mock![$$$]'";

/** Mock imports in test files. */
export const MOCK_TEST_RULES =
  "id: mockall-import\nlanguage: rust\nseverity: warning\nmessage: 'use mockall::...; mock import'\nrule:\n  kind: use_declaration\n  regex: 'mockall::'\n---\nid: faux-import\nlanguage: rust\nseverity: warning\nmessage: 'use faux::...; mock import'\nrule:\n  kind: use_declaration\n  regex: 'faux::'";
