# Normative ledger — initial records

Status: initial source extraction using the protocol approved in Q194.
The IDs and record format are adopted. This is not a complete CSS
ledger, completed implementation audit, or conformance result.

Common source for the records below: retained CSS 2.2, First Public Working Draft
12 April 2016, [§3.2](../tmp/css22/spec/conform.html#conformance). Exact file hash is
in [normative-source-baseline.md](normative-source-baseline.md). All rows are
CSS 2.2 source obligations; none imports newer authored syntax.

## Initial source records

| ID | Precise locator within §3.2 | Independently checkable obligation | Applicability | Primary roadmap owner |
| --- | --- | --- | --- | --- |
| CSS22-000001 | General conformance list, item 1 | Recognize at least one CSS 2.2 media type. | This project selects screen under Q006. | 49 |
| CSS22-000002 | General list, item 2, first sentence | Attempt retrieval of associated stylesheets appropriate to recognized media. | Applicable associated stylesheet inputs; acquisition stage depends on mode. | 62 |
| CSS22-000003 | General list, item 2, second sentence | When not all associated stylesheets can be retrieved, display using retrievable ones. | Rendering case; distinguish runtime behavior from approved Compile acquisition failure policy. | 65 |
| CSS22-000004 | General list, item 3, first two sentences | Parse stylesheets according to CSS 2.2, recognizing its at-rules, blocks, declarations, and selectors. | All managed stylesheet input. Needs detailed syntax child records. | 49 |
| CSS22-000005 | General list, item 3, supported-property value sentences | Accept valid values for properties applicable to supported media. | Per-property applicability; missing implementation is not an unknown-property exemption. | 49 |
| CSS22-000006 | General list, item 3, invalid-value clause | Ignore declarations whose property values are invalid. | Apply the specified recovery boundary; requires property/syntax child records. | 49 |
| CSS22-000007 | General list, item 3, final sentence | Ignore rules applying to unsupported media types. | Screen-only output; syntax handling remains separately required. | 49 |
| CSS22-000008 | General list, item 4 | Assign each element a value for every property according to definitions, cascade, and inheritance. | Managed source tree and property applicability. Needs property and interaction children. | 51 |
| CSS22-000009 | General list, item 5 | Permit user selection among alternate stylesheet sets when supplied by the source document. | Conditional on source-provided sets; source/API mapping remains under design. | 71 |
| CSS22-000010 | General list, item 6 | Permit the user to disable author stylesheet influence. | All supported modes, preserving origins in Compile artifacts. | 71 |
| CSS22-000011 | Rendering-UA exception list, third item | Render according to applicable media-specific CSS requirements in addition to general requirements. | Screen renderer; normative algorithms need child records. | 45 |
| CSS22-000012 | Paragraph beginning with user stylesheet file requirement | Allow the user to specify a user stylesheet file, subject to the stated device exemption. | Required native file-capable targets; Q045 handles build-time user inputs in Compile. | 71 |

## Design and evidence attached to these records

For all twelve records, source classification is normative and source text was
inspected locally. Evidence status is `specified` at the obligation level;
implementation and execution are unverified. Detailed algorithms and child records
are not implied by that status. These IDs do not constitute a coverage count
for all of CSS.

| Records | Existing approved design basis | Dependencies / required case families | Remaining design or evidence |
| --- | --- | --- | --- |
| 000001, 000007 | Q006; sections 4/14 | Media lists/import contexts; applicable and unsupported media cases. | Full media/source-clause extraction and parser fixtures. |
| 000002, 000003 | Sections 9/15–19 | Acquisition reasons, relative imports, missing resource and retained-active cases, mode-specific build behavior. | Detailed resolver/publication contracts and mode integration evidence. |
| 000004–000006 | Section 4 | Syntax recovery, selectors, property grammars, value validity, at-rule boundaries. | Complete clause/grammar crosswalk and valid/invalid input evidence. |
| 000008 | Sections 4/5/11/13 | Initial/inherit, origins/importance/order, dependencies and used-value stages. | Complete value/formatting/inheritance algorithms and cross-property cases. |
| 000009, 000010, 000012 | Sections 12/15/18 | Source-associated sets; author disable; user/author-important conflict; compiled origins. | Exact host/build/application APIs and corresponding executed fixtures. |
| 000011 | Sections 5–8/14 | Formatting, paint, input, resource, platform and renderer obligations. | Split into subsystem requirements; no aggregate verification claim. |

The case families are verification planning inputs, not existing test names or
executed results. Each record must acquire concrete cases and evidence through
its owner. Architecture decisions cited here establish direction, not product
support.

## Unclassified and referenced material

The remainder of §3.1–3.4 and the other chapter regions are not declared fully
audited by this initial set. Definitions, permissions, exceptions, and referenced
specifications require separate records or classified support/exclusion entries.
In particular, the device limitation clause, native form-control boundary, and
text/css transport references must not be silently converted into product
exemptions or modern transport policies. Their existing approved scope and any
unresolved details remain explicit in the source/roadmap records.

## Section-74 continuation

[Encoding records](normative-encoding-ledger.md) add CSS22-000013 through
CSS22-000029 from inspected §4.4. There are now 29 explicitly numbered CSS records
across these two files, still far short of a complete clause audit. The original
twelve records remain unchanged; no parent is closed by this additional extraction.

## Definition and scope continuation

[Chapter3 definitions and boundaries](normative-definitions-ledger.md) add
CSS22-000030 through000070. The current explicit total is70; the preceding29-record
checkpoint is historical. Source extraction is not complete CSS conformance.

## Syntax, selectors and cascade continuation

[Syntax records](normative-syntax-ledger.md) allocate CSS22-000071 through000184;
[selector/cascade records](normative-selector-cascade-ledger.md) allocate000185
through000304. Current total304 is a source-record count, not a conformance score.
Their unresolved interpretations and referenced definitions remain explicit.

## Painting, UI and media continuation

[Painting/UI/media records](normative-paint-ui-media-ledger.md) allocate
CSS22-000305 through000527. Current total527 supersedes earlier checkpoints.
These are specified source records and planned assertions, not executed tests.
The remaining chapter and reference audits stay open.

## Box-model continuation

[Box-model records](normative-box-ledger.md) allocate CSS22-000528 through000799:
272 new records, current total799. This includes all box chapter property fields
and semantic traversal, plus the explicit §9.9.1 subset resolving fixed-auto
context creation. Remaining visual-formatting/dimension clauses are not implied
complete by this count.

## Visual-formatting continuation

[Visual-formatting records](normative-visual-formatting-ledger.md) allocate
CSS22-000800 through001005, adding206 records. Current total1005. The full visuren
chapter traversal includes named exclusions and reused §9.9 records. Dimensions,
text, generated-content, tables and reference obligations remain open.

## Dimension continuation

[Dimension/line-metric records](normative-dimensions-ledger.md) add233 records,
CSS22-001006–001238; current total1238. This completes the local visudet traversal
with named exclusions and unresolved solver interpretations. It does not establish
all cross-clause interactions or close the remaining chapter/reference audits.

## Font and text continuation

[Font records](normative-font-ledger.md) add112 entries1239–1350;
[text/whitespace records](normative-text-ledger.md) add111 entries1351–1461.
Current total1461 is a source-record inventory. Generated-content/table and other
remaining source/reference/interaction work stays open; no execution is inferred.

## Generated-content and table continuation

[Generated-content records](normative-generated-ledger.md) add122 entries1462–1583;
[table records](normative-table-ledger.md) add172 entries1584–1755. Current total1755.
The major rendering chapters have now been traversed, with remaining source regions,
reference imports, policy links and source inconsistencies still explicitly open.

## Conventions and page-scope continuation

[Conventions/page records](normative-conventions-page-ledger.md) add82 entries1756–1837.
Current inventory1837; these are source records and planned cases, not executed
CSS tests. Five page properties remain in parsing/cascade scope while the selected
continuous-screen host excludes pagination. Appendix/reference/interaction closure
remains open.
