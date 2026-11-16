# Retained sources

Copies of the documents the conformance tests read and hash, kept here so the tests run
from a fresh checkout. Paths mirror the working `docs/` tree the manifest in
`../sources.json` names them by. Do not edit them: `../sources.json` pins each file's
SHA-256, and `tests/intake.rs` fails on any difference.

- `arch/`: this project's own architecture ledgers and property tables, as of the hashes
  in `../sources.json`.
- `tmp/css22/spec/`: pages of the W3C CSS 2.2 specification
  (<https://www.w3.org/TR/CSS22/>), unmodified. They are W3C documents and remain under
  the W3C Document License (<https://www.w3.org/copyright/document-license/>), not this
  repository's licence.
- `tmp/css22/property-definitions.json`: the property definition tables extracted from
  those pages.
- `tmp/implementation-default-newline-case/`: the input, the expected result and the
  recorded result of the one conformance case in `../cases.json`. The rest of that run's
  record (source snapshot, logs, tool versions) is not kept here, and `../cases.json`
  lists only these three as its verified artifacts.
