# Normative ledger continuation — stylesheet encoding

Status: inspected CSS22 §4.4 obligations; specified, implementation unverified.
Source: retained [syndata.html#charset](../tmp/css22/spec/syndata.html#charset),
fingerprinted in normative-source-baseline.md. IDs continue the twelve initial
records without reuse. All are children supporting CSS22-000004; source classification
is normative requirement/definition except the explicitly identified permissions
and recommendations. All apply to stylesheet intake in Compile, Manual and Auto.

Primary design owner: 49; collaborating owners: 62 for transport metadata, 64/65
for failure/publication, 66 for source coordinates and 72 for cases. Approved
design basis is section49's byte-first decode boundary. These source records do
not certify its unimplemented decoder or select an unreviewed charset library.

| ID | Precise §4.4 locator | Independently checkable obligation / permission | Required case and semantic assertion |
| --- | --- | --- | --- |
| CSS22-000013 | Opening two paragraphs | Stylesheets are UCS character sequences; stored/transmitted encoding supports the US-ASCII character set | ENC-CHARACTERS: preserve decoded characters and original byte provenance |
| CSS22-000014 | Paragraph beginning “When a style sheet is embedded” | Embedded CSS shares its containing document encoding | ENC-EMBEDDED: supplied source-language encoding governs embedded input |
| CSS22-000015 | Ordered list of five priorities | Separate-file detection precedence is transport parameter, BOM/charset detection, linking metadata, referring source encoding, then UTF-8 | ENC-PRECEDENCE: conflicting candidates at each adjacent priority; apply subsequent BOM exceptions |
| CSS22-000016 | Paragraph beginning “Authors using an @charset” | Authored charset declaration occurs at the beginning, with only an appropriate BOM allowed before it | ENC-POSITION: space/comment before charset does not act as detection declaration |
| CSS22-000017 | Paragraph beginning “@charset must be written literally” | Detection spelling is literal lowercase prefix, double-quoted encoding name and semicolon; no escapes | ENC-LITERAL: case, quote, escape and delimiter variations |
| CSS22-000018 | Paragraph beginning “The name must be a charset name” | Name comes from IANA charset registry; preferred MIME names are recommended to authors | ENC-NAME: retained registry name/alias resolution; registry edition/input still needs intake |
| CSS22-000019 | Paragraph beginning “User agents must support at least” | UTF-8 support is mandatory | ENC-UTF8: valid multibyte input and source-coordinate mapping |
| CSS22-000020 | Paragraph beginning “If rule 1 above” and following five-row table | Generic transport UTF-8/16/32 is overridden by recognized initial BOM according to that table | ENC-TRANSPORT-BOM: every table row and conflicting generic transport label |
| CSS22-000021 | Paragraph beginning “If rule 1 yields” | Explicit-endian UTF-16BE/LE or UTF-32BE/LE plus BOM is an error; recover by ignoring the specified encoding and using the preceding table | ENC-ENDIAN-ERROR: diagnostic and recovered bytes agree |
| CSS22-000022 | Paragraph beginning “User agents must ignore any @charset” | Charset rules outside the beginning are ignored | ENC-LATE: retain later valid style declarations without changing decoder |
| CSS22-000023 | First detection-list item | Except for the specified detection rules, ignore charset rules | ENC-NONDETECTION: no ordinary cascade effect from charset |
| CSS22-000024 | Second detection-list item, before table | Recommended byte-prefix detection chooses first matching table row and shortest repetition for name bytes | ENC-PREFIX: overlapping prefixes and shortest terminating quote/semicolon |
| CSS22-000025 | Same item, “User agents may ignore entries” | Entries relevant only to unsupported encodings may be ignored | ENC-UNSUPPORTED-TABLE: supported-encoding registry controls admissible detection entries; permission is not arbitrary UTF-8 lossy decoding |
| CSS22-000026 | Detection table XX/YY explanation | Name bytes follow specified order and required ASCII transcoding where marked | ENC-NAME-BYTES: matched entries decode the actual name rather than raw-byte guessing |
| CSS22-000027 | Detection table final “analogous patterns” row | UA may add analogous patterns for supported encodings absent from the table | ENC-EXTENSION: any selected extension is separately recorded; none is assumed by this ledger |
| CSS22-000028 | Third detection-list item | For an “as specified” result, ignore stylesheet unless the chosen decoding yields an appropriate initial charset rule | ENC-REDECODE: mismatched declaration encoding, BOM incompatibility and newline in name cannot bypass validation |
| CSS22-000029 | Final paragraph before §4.4.1 | Ignore stylesheets in unknown encodings | ENC-UNKNOWN: no replacement-character reinterpretation; mode-specific diagnostic/build policy remains separate |

Source permissions and should-level recommendations preserve their original force;
recording them does not turn every table encoding into a mandatory decoder. Exact
table bytes remain in the retained source instead of a second hand-transcribed
copy. Malformed-byte recovery, supported charset catalogue/aliases, and the IANA
source are outstanding detailed design/intake work under 49/62. They must be
settled before decoder implementation and section74 closure.

No ENC case above has run against a product parser. Full §4 lexical/recovery
extraction, all other chapters and external reference dispositions remain open.

## Implementation continuation: registry intake

The previously outstanding IANA name/alias source intake is now recorded in
[charset registry intake](charset-registry-intake.md). Bounded identity lookup is
implemented; this does not close the supported decoder catalogue or the other
encoding/qualification obligations above. No ledger requirement is marked passed.
