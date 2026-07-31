# Changelog

## [4.2.1](https://github.com/Falconiere/toolu/compare/v4.2.0...v4.2.1) (2026-07-31)


### Bug Fixes

* **toolu:** make the agent-tier pre-tool hook executable ([#143](https://github.com/Falconiere/toolu/issues/143)) ([5e7f112](https://github.com/Falconiere/toolu/commit/5e7f1124a105b65b383e0611f687f141aff25153))

## [4.2.0](https://github.com/Falconiere/toolu/compare/v4.1.1...v4.2.0) (2026-07-31)


### Features

* **toolu:** workflow harness v2 — unified verdict, telemetry, hybrid enforcement ([#141](https://github.com/Falconiere/toolu/issues/141)) ([66a0a71](https://github.com/Falconiere/toolu/commit/66a0a7139a45dd83755e05014d9957829ff4b628))

## [4.1.1](https://github.com/Falconiere/toolu/compare/v4.1.0...v4.1.1) (2026-07-30)


### Bug Fixes

* **pr-babysit:** catch replied-but-unresolved threads independent of last-comment filter ([#138](https://github.com/Falconiere/toolu/issues/138)) ([cabb726](https://github.com/Falconiere/toolu/commit/cabb726686e55aa2d36b10d69ef9c1e528d7c194))

## [4.1.0](https://github.com/Falconiere/toolu/compare/v4.0.1...v4.1.0) (2026-07-29)


### Features

* **toolu:** route delegated work to a model tier by task complexity ([#137](https://github.com/Falconiere/toolu/issues/137)) ([6875c89](https://github.com/Falconiere/toolu/commit/6875c89c4be90b49b3f448e4e953e0b09b399010))


### Bug Fixes

* **pr-babysit:** strictly clear every PR comment each tick ([#135](https://github.com/Falconiere/toolu/issues/135)) ([0c488cd](https://github.com/Falconiere/toolu/commit/0c488cda5eeb5643c7648f8d73e36432d21822df))

## [4.0.1](https://github.com/Falconiere/toolu/compare/v4.0.0...v4.0.1) (2026-07-28)


### Bug Fixes

* **toolu:** gate pushes on the target repo, not the hook's cwd ([#133](https://github.com/Falconiere/toolu/issues/133)) ([adfe24c](https://github.com/Falconiere/toolu/commit/adfe24c929e750c6cad77fc132c145ac47e892ff))

## [4.0.0](https://github.com/Falconiere/toolu/compare/v3.3.0...v4.0.0) (2026-07-27)


### ⚠ BREAKING CHANGES

* pi and opencode are no longer supported hosts. Installs made via `pi install` or tooling/install-opencode.sh will not receive updates; the `pi` package manifest, the @earendil-works/pi-coding-agent dependency, and the opencode adapter are gone. Claude Code is the only supported host.

### Bug Fixes

* correct and speed up the quality gates; drop pi and opencode ([#131](https://github.com/Falconiere/toolu/issues/131)) ([47d7954](https://github.com/Falconiere/toolu/commit/47d79540dd585a41f6a209a5efa307c57ddbdd45))

## [3.3.0](https://github.com/Falconiere/toolu/compare/v3.2.1...v3.3.0) (2026-07-20)


### Features

* **agent-browser:** token-lean browser automation plugin (skill + CLI wrapper) ([#129](https://github.com/Falconiere/toolu/issues/129)) ([9ffdac3](https://github.com/Falconiere/toolu/commit/9ffdac3567e1d82612b3a26ed1c5562712c6b325))

## [3.2.1](https://github.com/Falconiere/toolu/compare/v3.2.0...v3.2.1) (2026-07-20)


### Bug Fixes

* **ts-quality:** gate the ../ import rule on a @/ alias, clean the store's as/catch ([#127](https://github.com/Falconiere/toolu/issues/127)) ([1c63157](https://github.com/Falconiere/toolu/commit/1c6315740ff314464f547e8a5d5c183ca6fd0ae6))

## [3.2.0](https://github.com/Falconiere/toolu/compare/v3.1.0...v3.2.0) (2026-07-20)


### Features

* **toolu:** add exa-search and context7 proactive-use mandates ([aed41a3](https://github.com/Falconiere/toolu/commit/aed41a30992adbee73776397700c4c92f6d329e5))


### Bug Fixes

* **comemory:** cap the symlink walk and cover relative link chains ([4f7becd](https://github.com/Falconiere/toolu/commit/4f7becd6676d11d052dc62d25c28b304db2a287d))
* **comemory:** resolve the wrapper's own path through symlinks ([e50f1a1](https://github.com/Falconiere/toolu/commit/e50f1a1da705853297dcd310b248c8801f66ef69))
* **dashboard:** address review — derive test clock from fixture, JSDoc the param ([4b88852](https://github.com/Falconiere/toolu/commit/4b88852771f329e80fc2b465d9fe7a784991c5de))
* **dashboard:** thread the injected clock through session retention ([02cc295](https://github.com/Falconiere/toolu/commit/02cc295d21f201f7387422b4eb40461ecbf2e8f8))
* **toolu:** address PR review feedback ([0ae9f6d](https://github.com/Falconiere/toolu/commit/0ae9f6d8fc7831678de332ad7fe2920491296785))
* **toolu:** point the recall hint at the published wrapper path ([a426915](https://github.com/Falconiere/toolu/commit/a426915a2de5c2f788a222e13572e35e540d761e))

## [3.1.0](https://github.com/Falconiere/toolu/compare/v3.0.0...v3.1.0) (2026-07-09)


### Features

* **jira:** add plan family to decompose ticket work ([e0d0316](https://github.com/Falconiere/toolu/commit/e0d031674aad061a0feca6a7ef7cd1f393962736))


### Bug Fixes

* **dashboard:** surface dropped config warning ([5cf58e4](https://github.com/Falconiere/toolu/commit/5cf58e4f029f615b36054a30a805946ffc698116))
* **jira:** validate the issue key before building any path ([03618c3](https://github.com/Falconiere/toolu/commit/03618c33c4432cdb87070dfdcfe2c94947857f96))
* **pr-babysit:** parse the review bot's current verdict label ([375d648](https://github.com/Falconiere/toolu/commit/375d64842006f93533e740880c007d9e7ef38af4))

## [3.0.0](https://github.com/Falconiere/toolu/compare/v2.1.1...v3.0.0) (2026-07-06)


### ⚠ BREAKING CHANGES

* **comemory:** installing toolu no longer auto-installs caveman or code-simplifier — install them yourself if you want them. The comemory persistent-memory mandate is now opt-in: run /comemory:setup once per repo to enable recall/save; users who relied on auto-activation must run it.

### Features

* **comemory:** make comemory opt-in via /comemory:setup; drop caveman + code-simplifier to optional ([c7a25bf](https://github.com/Falconiere/toolu/commit/c7a25bff78589ff723d27e2977847546de352197))


### Bug Fixes

* **comemory:** suppress setup nudge on explicit setup_done=false ([fbccb6e](https://github.com/Falconiere/toolu/commit/fbccb6e41515865cf18e3f939c18fa4cd616da11))
* **comemory:** tighten path-parity assertion + add resolver drift guards ([e41725d](https://github.com/Falconiere/toolu/commit/e41725d5c133230b4fb6dad921191521bcfc0b1c))

## [2.1.1](https://github.com/Falconiere/toolu/compare/v2.1.0...v2.1.1) (2026-06-19)


### Bug Fixes

* **pr-babysit:** add GraphQL no-suffix claude form to CI_REVIEWER ([427011a](https://github.com/Falconiere/toolu/commit/427011a46a968ab2707875f478a7e5aa4b84e354))
* **pr-babysit:** reply to CI bot inline threads, drop summary comments ([f7f38ba](https://github.com/Falconiere/toolu/commit/f7f38baa6649f04d633b73677088188ca4ebe026))

## [2.1.0](https://github.com/Falconiere/toolu/compare/v2.0.0...v2.1.0) (2026-06-19)


### Features

* **plan-ledger:** enrich step schema with ac_refs, retries, depends_on, input ([e0688a3](https://github.com/Falconiere/toolu/commit/e0688a395bf9b4190054f6a84eada85ee0d4adbc))

## [2.0.0](https://github.com/Falconiere/toolu/compare/v1.23.0...v2.0.0) (2026-06-18)


### ⚠ BREAKING CHANGES

* **design:** the design-review skill is renamed to design and is now invoked as /design <command> [target].

### Features

* **debug:** add break-glass debug skill with evidence helpers ([7f3be16](https://github.com/Falconiere/toolu/commit/7f3be166acaf4c859eada7dbea3fd70245b8d855))
* **design:** replace review-only skill with /design command dispatcher ([d2c227e](https://github.com/Falconiere/toolu/commit/d2c227ed9c34cd00b40e4d0a3dd7790c7004b98d))


### Bug Fixes

* bash 3.2 portability (mapfile) and test env isolation ([80d0df9](https://github.com/Falconiere/toolu/commit/80d0df999c6066bbab75ec479d69e75ba5207a1a))

## [1.23.0](https://github.com/Falconiere/toolu/compare/v1.22.0...v1.23.0) (2026-06-18)


### Features

* **statusline:** show git working-tree status and ahead/behind in statusline ([a3915f0](https://github.com/Falconiere/toolu/commit/a3915f037beeb2f942da4ac40475cbb5c394a2fd))


### Bug Fixes

* **statusline:** use explicit -b main in ahead/behind tests for CI compat ([c4c5ef7](https://github.com/Falconiere/toolu/commit/c4c5ef7ba076af2cb53bb222e82bd941b11a3244))

## [1.22.0](https://github.com/Falconiere/toolu/compare/v1.21.0...v1.22.0) (2026-06-18)


### Features

* **design:** add design plugin with stack-aware design-review skill ([5fc238c](https://github.com/Falconiere/toolu/commit/5fc238c3d448f836939f75181a904eb5ae1c99dd))

## [1.21.0](https://github.com/Falconiere/toolu/compare/v1.20.0...v1.21.0) (2026-06-17)


### Features

* **jira:** nudge Claude to the jira skill over the Atlassian MCP ([10f06ae](https://github.com/Falconiere/toolu/commit/10f06aee0320e998eb8cc28cf565134996a552a6))

## [1.20.0](https://github.com/Falconiere/toolu/compare/v1.19.0...v1.20.0) (2026-06-17)


### Features

* **opencode:** add opencode adapter, install script, and opencode-format agents/commands ([af54ca9](https://github.com/Falconiere/toolu/commit/af54ca96bd56e5a79b784da19a6f8f01188df348))
* **tooling:** --dry-run plan, auto-draft release notes, wire curated notes into GitHub Release body ([#84](https://github.com/Falconiere/toolu/issues/84)) ([3943741](https://github.com/Falconiere/toolu/commit/3943741eb8dd22c18f730c0c59fdcb5f80deab07))
* **tooling:** automate releases with release-please ([29fc41f](https://github.com/Falconiere/toolu/commit/29fc41f8c1ac54baf3b504c17047f01dbf081c39))


### Bug Fixes

* **opencode:** address review comments on the opencode port ([1706d44](https://github.com/Falconiere/toolu/commit/1706d44571c3a8bc1e496105c5553742943c229a))
* **opencode:** guard unbound TOOLU_CONFIG_DIR in install script ([6111a7a](https://github.com/Falconiere/toolu/commit/6111a7a584bd3166a988f3810de55c0593f412af))
* **release:** check out repo so the package.json sync step has files ([fce0dc4](https://github.com/Falconiere/toolu/commit/fce0dc41465bc86684eea7115e67523cc2941c53))
* **release:** switch release-please to whole-repo single-version mode ([31b9d8a](https://github.com/Falconiere/toolu/commit/31b9d8ad543de8dda6602cd2a77d42f30f2ae12c))
