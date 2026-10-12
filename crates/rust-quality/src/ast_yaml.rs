//! The inline ast-grep rules. Severity is `warning` on purpose: `scan` exits
//! non-zero on an error-level match, which would be indistinguishable from a
//! crashed tool.

/// Panic-on-error rules for production code.
pub(crate) const ERROR_RULES: &str = "id: unwrap\nlanguage: rust\nseverity: warning\nmessage: unwrap on Result/Option\nrule:\n  pattern: '$E.unwrap()'\n---\nid: expect\nlanguage: rust\nseverity: warning\nmessage: expect on Result/Option\nrule:\n  pattern: '$E.expect($M)'\n---\nid: panic\nlanguage: rust\nseverity: warning\nmessage: panic!\nrule:\n  pattern: 'panic!($$$)'\n---\nid: todo\nlanguage: rust\nseverity: warning\nmessage: todo!\nrule:\n  pattern: 'todo!($$$)'\n---\nid: unimplemented\nlanguage: rust\nseverity: warning\nmessage: unimplemented!\nrule:\n  pattern: 'unimplemented!($$$)'\n---\nid: unreachable\nlanguage: rust\nseverity: warning\nmessage: unreachable!\nrule:\n  pattern: 'unreachable!($$$)'";

/// Mock definitions in src/.
pub(crate) const MOCK_SRC_RULES: &str = "id: automock-attr\nlanguage: rust\nseverity: warning\nmessage: '#[automock] mock definition'\nrule:\n  pattern: '#[automock]'\n---\nid: automock-cfg-attr\nlanguage: rust\nseverity: warning\nmessage: cfg_attr(..., automock) mock definition\nrule:\n  pattern: '#[cfg_attr($$$, automock)]'\n---\nid: mock-macro\nlanguage: rust\nseverity: warning\nmessage: 'mock! { ... } mock definition'\nrule:\n  any:\n    - pattern: 'mock! { $$$ }'\n    - pattern: 'mock!($$$)'\n    - pattern: 'mock![$$$]'";

/// Mock imports in test files.
pub(crate) const MOCK_TEST_RULES: &str = "id: mockall-import\nlanguage: rust\nseverity: warning\nmessage: 'use mockall::...; mock import'\nrule:\n  kind: use_declaration\n  regex: 'mockall::'\n---\nid: faux-import\nlanguage: rust\nseverity: warning\nmessage: 'use faux::...; mock import'\nrule:\n  kind: use_declaration\n  regex: 'faux::'";
