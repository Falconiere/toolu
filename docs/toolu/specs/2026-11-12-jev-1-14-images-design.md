# Jev 1.14: images, Box answers and long context — Design

**Date:** 2026-11-12   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** Bring the jev plugin to the Jev 1.14 API contract

## Problem

TypeSafe released Jev 1.14 on 2026-10-14 and `jev-latest` already resolves to
`jev-1.14.0`. The jev plugin still speaks the text-only 1.13 contract:

- An agent cannot give Jev an image, so a screenshot or a photo cannot be judged.
- `toolu jev ask` with a `"type": "box"` question fails with `invalid response`,
  because the reply check accepts only `noul`, `choice` and `score`.
- The 262k-token long context cannot be turned on.
- The skill and two READMEs say "Text input only", and the skill's question
  guidance predates the 1.14 advice.

## Non-Goals

1. A `toolu jev box` verb. Box goes through `ask`, next to a Noul for presence.
2. A `toolu jev models` verb for `GET /v1/models`.
3. Multipart upload and gzip or zstd request bodies.
4. Local pixel-dimension checks or resizing. The API rejects a bad image and
   its body is printed.
5. Changing `OPENROUTER_MODEL` (`typesafe/jev-1.13`) or any code-review threshold.
6. Changing the SessionStart or UserPromptSubmit mandate text.

## Architecture

The change is additive. `Jev::ask`, `Jev::ask_until` and `request_body` keep
their signatures, and a request without images has the same body text as
before. That matters because code-review uses `request_body` text as its replay
key and four crates call these functions.

- `crates/core/jev` (`toolu-jev-client`) gains `Assets`, `Extras`, `Jev::ask_with`
  and `Answer::Box`.
- `crates/jev` (`toolu-jev`) gains two flags on every verb and passes an
  `Extras` to `ask_with`.
- Images travel base64-encoded inside the JSON body. The trade-off: base64 makes
  an image a third larger against the API's 32 MiB body cap, in exchange for no
  multipart encoder and one request text that can still be recorded. About
  24 MiB of images fit in one request.
- The two opt-in headers are derived from the request, not passed by the
  caller: images for a request with an asset or a Box question, long context
  for `Extras::long_context`.

## Interfaces / Schema

CLI, on `noul`, `choice`, `score` and `ask`:

```text
-i, --image <REF=PATH>   repeatable; split at the first `=`
    --long-context
```

Client (`toolu-jev-client`):

```rust
pub struct Assets(/* ordered (reference, asset) entries */);
impl Assets { pub fn image(&mut self, reference: &str, bytes: &[u8]) -> Result<(), Error>; }
pub struct Extras { pub assets: Assets, pub long_context: bool }   // Default
impl Jev { pub fn ask_with(&self, &State, &str, &Questions, &Extras) -> Result<Reply, Error>; }
pub enum Answer { /* … */ Box { top: f64, bottom: f64, left: f64, right: f64, confidence: Option<f64> } }
```

Request body, with assets after the questions and only when there are any:

```json
{"state": "shot-7f3a", "model": "jev-latest", "questions": {"q": {"type": "noul", "instructions": "…"}},
 "assets": {"shot-7f3a": {"type": "image", "format": "png", "data_base64": "…"}}}
```

`format` is `png` or `jpg`, read from the file's first bytes. Headers, lower
case: `typesafe-danger-zone-images: true`, `typesafe-danger-zone-long-context: true`.

Box reply: `{"type": "box", "box": {"top", "bottom", "left", "right"}, "confidence": number | null}`.

## Failure modes and edge cases

| Input | Behavior |
| --- | --- |
| `--image shot` (no `=`) | exit 1, `jev: --image needs REF=PATH`, no request |
| `--image shot=/missing.png` | exit 1, `jev: cannot read /missing.png`, no request |
| File that is neither PNG nor JPEG, or empty | exit 1, `jev: image shot is not PNG or JPEG`, no request |
| File over 16 MiB | `Error::InvalidQuestion`, no request |
| Blank, repeated, or over-1,024-byte reference | `Error::InvalidQuestion`, no request |
| Image outside 280–1568 px, or a reference written nowhere | the API's 4xx; its body on stderr, exit 1, not retried |
| Box reply with an edge outside [0, 1], a missing edge, or no `confidence` key | `Error::InvalidResponse`; no judgment is printed |
| Box reply with `confidence: null` | accepted; `confidence` is `None` |
| Request without images or Box | body and headers unchanged from before |

Every refusal propagates as an error to the caller; none is recovered. There is no
concurrent case: each call builds and sends one request and shares no state.

## Acceptance criteria

- **AC-1:** Given `toolu jev noul -s shot-7f3a "…" -i shot-7f3a=<a real PNG file>`, the endpoint receives `assets.shot-7f3a` equal to `{"type":"image","format":"png","data_base64":<the file's base64>}`, the header `typesafe-danger-zone-images: true`, and no long-context header.
- **AC-2:** Given `toolu jev ask - -s photo --long-context` with a Box question on stdin and a Box reply whose `confidence` is `null`, the command exits 0 and prints the Box answer, and the request carries both opt-in headers and no `assets`.
- **AC-3:** Given `--image shot`, `--image shot=/nonexistent/shot.png` and `--image shot=<a text file>`, each run exits 1 with the stderr line named in the failure table and the endpoint receives no request.
- **AC-4:** Given a text-only Noul request, the body is exactly `{"state":…,"model":…,"questions":…}` with no `assets` key and neither opt-in header is sent.
- **AC-5:** Given a Box question, a reply with edges in [0, 1] is read as `Answer::Box`, and a reply with an edge of 1.2, a missing edge, an array for `box`, a confidence of 1.5, or no `confidence` key is `Error::InvalidResponse`.
- **AC-6:** Given the refusal inputs for `Assets::image` (blank, repeated and 1,025-byte references; `GIF89a` bytes; empty bytes; a file of 16 MiB + 1 byte), each returns its reason and the assets are unchanged, while a 1,024-byte reference is accepted.
- **AC-7:** Given the repository's Markdown and generated CLI reference, `cargo xtask check-markdown-cli` and `cargo xtask docs-cli --check` pass, and the CLI contract snapshot matches the binary.

## Acceptance evidence

| AC | Real input | Expected result | Boundary or failure case | Check |
| --- | --- | --- | --- | --- |
| AC-1 | The 16×16 PNG of TypeSafe's Box example, written to a temp file; the real binary's verb against the loopback TLS origin | exact `assets` JSON and the images header | long-context header absent | `cargo nextest run -p toolu-jev --test images an_image_travels_under_assets_and_turns_images_on` |
| AC-2 | A Box question on stdin; a `jev-1.14.0` Box reply | printed answers equal the reply's `answers` | `confidence: null`; no `assets` key | `cargo nextest run -p toolu-jev --test images ask_prints_a_box_answer_and_long_context_sends_its_header` |
| AC-3 | Three bad `--image` values, one a real text file | exit 1 and the named stderr | zero requests received | `cargo nextest run -p toolu-jev --test images a_bad_image_fails_with_its_reason_before_any_request` |
| AC-4 | A Noul over text, through `ask` and `ask_with` | exact body text; no opt-in header | a Box question in the same test turns images on | `cargo nextest run -p toolu-jev-client --test cases noul_sends_one_typed_question_and_reads_its_answer` and `cargo nextest run -p toolu-jev-client --test images` |
| AC-5 | Box replies in the API's documented shape | typed `Answer::Box`, `None` for null confidence | five malformed replies | `cargo nextest run -p toolu-jev-client reply::tests::a_box_answer_keeps_its_edges_and_may_have_no_confidence` |
| AC-6 | The same PNG, a JPEG header, GIF bytes, a 16 MiB + 1 byte buffer | exact assets JSON; each refusal's reason | reference of exactly 1,024 bytes accepted | `cargo nextest run -p toolu-jev-client asset::tests` |
| AC-7 | `plugins/jev/skills/**`, `docs/**`, `docs/cli/commands.json` | both checks print ok; snapshot test passes | a flag the binary lacks would fail the Markdown check | `cargo xtask check-markdown-cli`, `cargo xtask docs-cli --check`, `cargo nextest run -p toolu-cli --test contract` |

Live evidence outside the gate, run on 2026-11-12 with the built binary against
`api.typesafe.ai`: an image Noul answered 0.93 on `jev-1.14.0`; a Box question
returned edges; a 2000 px and a 200 px image each returned
`Invalid image dimensions`; an unreferenced image returned
`Invalid image request`; a text Noul and a `--long-context` Noul answered as before.

## Documentation impact

- `plugins/jev/skills/jev/SKILL.md`: the two flags, Box through `ask`, an Images section, and the 1.14 question guidance.
- `plugins/jev/skills/jev/references/problem-solving.md`: one screenshot example.
- `plugins/jev/README.md`, `docs/jev/README.md`, `crates/core/jev/README.md`: input formats, image limits, Box, long context, unretried statuses.
- `docs/cli/jev.md`, `docs/cli/commands.json`, `crates/cli/tests/fixtures/commands_json.snap`: regenerated.
- Plugin descriptions in the three `plugins/jev/*-plugin/plugin.json` files and `.claude-plugin/marketplace.json`.

## Open Questions

- Should images move to multipart once a request needs more than about 24 MiB of images? Owner: maintainer. Not blocking: no current caller sends images.
- Should `OPENROUTER_MODEL` move to 1.14? Owner: maintainer. Not blocking: out of scope here and needs a threshold re-evaluation in code-review.
