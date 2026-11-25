---
name: jev-documents
description: Read documents with Jev — extract values and dates, recover structure from plain text, check citations against a source, classify passages or whole files, and read scanned pages or screenshots with Box plus a presence Noul. Use for invoices, reports, PDFs, OCR output, RAG passages, or any text/image that must become typed data.
---

# Documents with Jev

Jev reads what a human skims: does this page state X, which value is the
total, where in the image is the stamp. Code still parses, normalizes, and
stores. CLI grammar and image limits: [jev](../jev/SKILL.md).

## Select, don't generate

Pre-parse candidates and let Jev pick the intended one; never ask it to
regurgitate a value. For "the invoice total" (the image belongs to the
question, so its `REF` is written in the instructions):

```bash
toolu jev choice "Which candidate in `candidates` is the payable total shown in shot-1f?" \
  -s @state.json -i shot-1f=invoice.png \
  -o subtotal="the line before tax or discount" \
  -o total="the final amount due, after tax and discount" \
  -o gross="the amount before any discount"
```

Then normalize the chosen span in code (strip currency, fix the decimal).
Dates: ask Jev for the parts in a document (`issue_date`, `due_date`, `terms`)
as Nouls or Choices, and resolve "Net 30" arithmetic in code, never in the
model.

## Recover structure from flat text

Send one `ask` with a per-line Choice ("heading / body / list item / code /
callout") plus a Noul on whether a wrap boundary is a real break. Rebuild the
Markdown in code from the returned labels.

## Check against the source

To verify a claim or a citation, pair the excerpt with the source document and
ask a Choice: does the source's context support, contradict, or fail to back
the quoted line? Put the quote and its source in the state so the question can
reference both by path; the judgment is one comparison, answerable at a glance.

## Scanned pages and screenshots

Box finds where something is; it always returns a box, so gate it with a
companion Noul in the same call — write the image `REF` in the state and ask
"Is a signature present in `shot-2b`?" — and ignore the box when the Noul is
low. Confirm each field with a text-only question ("Does the dose year in
`shot-2b` fall in 2020–2025?"). Images are judged at a distance: treat a
decisive visual read as provisional and verify by another path.

For long documents, slice into bounded sections rather than defaulting to
`--long-context`, which is slower and less accurate.

Values a model can't see (exact figures, IDs, counts) stay in code. A failed
call is a read that did not happen: record it, do not guess.
