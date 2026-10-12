//! The inline ast-grep rules, verbatim from 78-error-ast.sh and 85-no-mocks.sh.
//! Severity is `warning` on purpose: `scan` exits non-zero on an error-level
//! match, which would be indistinguishable from a crashed tool.

/// Error-handling rules: swallowed errors, silent rejections, non-Error throws.
pub(crate) const ERROR_RULES: &str = r#"id: empty-catch
language: ts
severity: warning
message: empty catch
rule:
  pattern: 'try { $$$ } catch ($_) { }'
---
id: empty-catch-noarg
language: ts
severity: warning
message: empty catch (no binding)
rule:
  pattern: 'try { $$$ } catch { }'
---
id: empty-catch-handler
language: ts
severity: warning
message: empty promise catch handler
rule:
  pattern: '$_.catch(() => { })'
---
id: null-catch-handler
language: ts
severity: warning
message: promise catch handler returning null
rule:
  pattern: '$_.catch(() => null)'
---
id: undef-catch-handler
language: ts
severity: warning
message: promise catch handler returning undefined
rule:
  pattern: '$_.catch(() => undefined)'
---
id: swallow-null-arg
language: ts
severity: warning
message: catch returns null
rule:
  pattern: 'try { $$$ } catch ($_) { return null }'
---
id: swallow-undef-arg
language: ts
severity: warning
message: catch returns undefined
rule:
  pattern: 'try { $$$ } catch ($_) { return undefined }'
---
id: swallow-null
language: ts
severity: warning
message: catch returns null (no binding)
rule:
  pattern: 'try { $$$ } catch { return null }'
---
id: swallow-undef
language: ts
severity: warning
message: catch returns undefined (no binding)
rule:
  pattern: 'try { $$$ } catch { return undefined }'
---
# A bare `return` in an ast-grep pattern acts as a WILDCARD over the optional
# argument: `catch { return }` also matched `return []`, `return 42` and
# `return null`, i.e. every non-nullish fallback got reported as "returns a
# nullish value". Match the catch clause itself, then require that its single
# return statement carries no argument at all.
id: swallow-bare
language: ts
severity: warning
message: catch returns nothing
rule:
  all:
    - any:
        - pattern:
            context: 'try {} catch { return }'
            selector: catch_clause
        - pattern:
            context: 'try {} catch ($_) { return }'
            selector: catch_clause
    - not:
        has:
          stopBy: end
          kind: return_statement
          has:
            stopBy: neighbor
            pattern: $X
---
id: throw-empty-error
language: ts
severity: warning
message: throw new Error() with no message
rule:
  pattern: 'throw new Error()'
---
id: throw-string
language: ts
severity: warning
message: throw of a string literal
rule:
  pattern: 'throw "$S"'
---
id: throw-template
language: ts
severity: warning
message: throw of a template literal
rule:
  pattern: 'throw `$S`'"#;

/// Mock-call rules for test files.
pub(crate) const MOCK_RULES: &str = r"id: jest-mock
language: ts
severity: warning
message: jest.mock() call
rule:
  pattern: jest.mock($$$)
---
id: vi-mock
language: ts
severity: warning
message: vi.mock() call
rule:
  pattern: vi.mock($$$)
---
id: jest-fn
language: ts
severity: warning
message: jest.fn() call
rule:
  pattern: jest.fn($$$)
---
id: vi-fn
language: ts
severity: warning
message: vi.fn() call
rule:
  pattern: vi.fn($$$)
---
id: sinon-method
language: ts
severity: warning
message: sinon mock method call
rule:
  pattern: sinon.$M($$$)";
