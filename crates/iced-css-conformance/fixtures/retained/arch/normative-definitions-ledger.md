# Normative ledger continuation — definitions and conformance boundaries

Status: inspected retained chapter3 text, including its scoped examples and notes.
CSS22-000030 through000070 supplement the earlier twelve §3.2 records without
replacing them. These are source definitions, obligations, permissions and scope
rules; they are not 41 implemented or passing CSS tests.

Source: [retained conform.html](../tmp/css22/spec/conform.html), CSS22 FPWD
12 April 2016, fingerprinted in [source baseline](normative-source-baseline.md).
Each locator below names the anchor and direct paragraph/definition. Source
classification follows that region’s text: permissions remain permissions and
informative scope markers classify their contents rather than require rendering.

Common evidence status: specified, implementation unverified. Design owners are
numbered architecture sections; the source/capture, cascade, formatting, resource
and API contracts provide direction, not execution evidence. Applicability is
managed screen sources in Compile/Manual/Auto unless the row explicitly describes
a conditional role/device permission. Native platforms and both backends retain
the same source semantics; backend incapacity is not a scope exclusion.

| ID | Retained locator | Obligation, definition or permission | Owner | Planned semantic case |
| --- | --- | --- | --- | --- |
| CSS22-000030 | [§3 #defs](../tmp/css22/spec/conform.html#defs); Opening requirement-word paragraph | Interpret requirement words using RFC2119 even when lowercase. | 45 | FORCE: uppercase/lowercase wording retains the same requirement level. |
| CSS22-000031 | [§3 #defs](../tmp/css22/spec/conform.html#defs); Good-practice recommendation paragraph | Recommendations identified as good practice are non-normative; conformance does not require them. | 45 | SCOPE-RECOMMENDATION: classify scoped recommendations separately from normative should statements. |
| CSS22-000032 | [§3 #defs](../tmp/css22/spec/conform.html#defs); Deprecation paragraph | Deprecation alone does not remove conformance obligations; normative system colors remain required. | 50 | DEPRECATION: do not reject system colors because they are marked for future deprecation. |
| CSS22-000033 | [§3 #defs](../tmp/css22/spec/conform.html#defs); All sections paragraph | Sections and appendices are normative unless explicitly stated otherwise. | 45 | SCOPE-DEFAULT: preserve default force outside each explicit exclusion. |
| CSS22-000034 | [§3 #defs](../tmp/css22/spec/conform.html#defs); Examples and notes paragraph | Examples and notes are non-normative. | 45 | SCOPE-EXAMPLE: example content is not promoted into an additional authored requirement. |
| CSS22-000035 | [§3 #defs](../tmp/css22/spec/conform.html#defs); Figures paragraph | Figures are illustrations, not reference renderings unless explicitly designated. | 72 | ORACLE-FIGURE: require explicit reference status before pixel comparison is normative. |
| CSS22-000036 | [§3 #style-sheet](../tmp/css22/spec/conform.html#style-sheet); Definition and origins paragraph | A stylesheet specifies presentation; distinguish author, user and UA origins in cascade. | 49 | ORIGIN: identical declarations at distinct origins retain their cascade role. |
| CSS22-000037 | [§3 #valid-style-sheet](../tmp/css22/spec/conform.html#valid-style-sheet); Validity paragraph and final invalid definition | Valid CSS22 uses its grammar and only defined at-rules, property names and values; invalidity is level-specific. | 49 | VALIDITY: syntax acceptance alone does not establish valid property values. |
| CSS22-000038 | [§3 #valid-style-sheet](../tmp/css22/spec/conform.html#valid-style-sheet); Opening compatibility sentences | Valid CSS1 sheets remain valid CSS22 with possible semantic changes; not every CSS2 sheet is valid CSS22. | 49 | LEVEL: avoid treating CSS2 or later feature names as automatically valid CSS22. |
| CSS22-000039 | [§3 #source-document](../tmp/css22/spec/conform.html#source-document); Definition | The source represents an element tree; each element has a type name, optional attributes and possibly empty content. | 47 | SOURCE: captured element metadata remains separate from generated formatting boxes. |
| CSS22-000040 | [§3 #doclanguage](../tmp/css22/spec/conform.html#doclanguage); Definition | CSS describes presentation without changing underlying document-language semantics. | 47 | SEMANTICS: display changes do not change source parentage or declared source roles. |
| CSS22-000041 | [§3 #element](../tmp/css22/spec/conform.html#element); Definition | Elements are the source language’s primary syntactic constructs, whose names may be used by style rules. | 47 | TYPE: selector type matching uses registered source names, not Rust display labels. |
| CSS22-000042 | [§3 #replaced-element](../tmp/css22/spec/conform.html#replaced-element); First and final paragraphs | Replaced-element contents are outside the CSS formatting/rendering model; they may have intrinsic dimensions or ratio. | 52 | REPLACED: outer CSS sizing and separate resource rendering have distinct responsibilities. |
| CSS22-000043 | [§3 #replaced-element](../tmp/css22/spec/conform.html#replaced-element); Sensitive intrinsic-dimensions paragraph | UA may withhold intrinsic dimensions believed to leak sensitive information. | 52 | PRIVACY-DIMS: permission needs an explicit policy; no automatic missing-dimensions excuse. |
| CSS22-000044 | [§3 #intrinsic](../tmp/css22/spec/conform.html#intrinsic); First three sentences | Intrinsic dimensions come from the element, not surroundings; their discovery is outside CSS; in CSS22 only replaced elements have them. | 52 | INTRINSIC: do not confuse non-replaced content measurements with replaced intrinsic dimensions. |
| CSS22-000045 | [§3 #intrinsic](../tmp/css22/spec/conform.html#intrinsic); Raster resolution sentence | Raster images without reliable resolution information use one CSS px per source image pixel. | 62 | RASTER-SIZE: absent resolution metadata maps a20x10 raster to20px by10px intrinsic dimensions. |
| CSS22-000046 | [§3 #attribute](../tmp/css22/spec/conform.html#attribute); Definition | An attribute has a name and associated textual value. | 47 | ATTRIBUTE: preserve source text values for selector evaluation. |
| CSS22-000047 | [§3 #content](../tmp/css22/spec/conform.html#content); Definition | Source content may contain text and subelements; no content means empty; the containing element is their parent. | 47 | CONTENT: empty source content is distinguished from hidden or unpainted content. |
| CSS22-000048 | [§3 #ignore](../tmp/css22/spec/conform.html#ignore); First meaning and linked recovery sections | Parser ignoring discards precisely the portions prescribed by the applicable syntax recovery rule. | 49 | IGNORE-SYNTAX: invalid declaration recovery does not discard unrelated valid declarations. |
| CSS22-000049 | [§3 #ignore](../tmp/css22/spec/conform.html#ignore); Second meaning | Properties/values can be disregarded by applicability rules even when their syntax is valid. | 51 | IGNORE-APPLICABILITY: table-column font behavior is not implemented by marking font syntax invalid. |
| CSS22-000050 | [§3 #rendered-content](../tmp/css22/spec/conform.html#rendered-content); Definition | Rendered content follows styles and may include replacements, alternate text and generated items; replaced internal rendering is unspecified. | 56 | RENDERED: generated markers do not mutate source child lists. |
| CSS22-000051 | [§3 #doctree](../tmp/css22/spec/conform.html#doctree); Definition including root | Every source element has exactly one parent except the root, which has none. | 47 | TREE: reject shared/cyclic source parentage without confusing native implementation slots with elements. |
| CSS22-000052 | [§3 #child](../tmp/css22/spec/conform.html#child); Definition | A is B’s child exactly when B is A’s parent. | 47 | CHILD: source child selectors use immediate parentage. |
| CSS22-000053 | [§3 #descendant](../tmp/css22/spec/conform.html#descendant); Definition | Descendant is the recursive child relation. | 47 | DESCENDANT: deeper descendants match; the element itself does not become its own descendant. |
| CSS22-000054 | [§3 #ancestor](../tmp/css22/spec/conform.html#ancestor); Definition | Ancestor is the inverse descendant relation. | 47 | ANCESTOR: source ancestry is unchanged by anonymous box creation. |
| CSS22-000055 | [§3 #sibling](../tmp/css22/spec/conform.html#sibling); Definition | Siblings share a parent; preceding/following siblings are distinguished by document-tree order. | 47 | SIBLING: layout reordering does not change source sibling order. |
| CSS22-000056 | [§3 #preceding](../tmp/css22/spec/conform.html#preceding); Definition | A preceding element here is an ancestor or preceding sibling, as explicitly defined in this section. | 47 | PRECEDING: do not substitute a generic flattened traversal relation for this defined term. |
| CSS22-000057 | [§3 #following](../tmp/css22/spec/conform.html#following); Definition | Following element is the inverse of the preceding-element definition. | 47 | FOLLOWING: use the source-defined inverse relation. |
| CSS22-000058 | [§3 #author](../tmp/css22/spec/conform.html#author); Definition including authoring tool | Author writes documents/stylesheets; an authoring tool is a UA generating stylesheets. | 45 | ROLE-AUTHOR: Compile consumption does not by itself claim authoring-tool conformance. |
| CSS22-000059 | [§3 #user](../tmp/css22/spec/conform.html#user); Definition | User interacts with the UA and may supply a personal stylesheet expressing preferences. | 71 | ROLE-USER: user origin is preserved independently of author defaults. |
| CSS22-000060 | [§3 #user-agent](../tmp/css22/spec/conform.html#user-agent); Definition and following HTML/XHTML paragraph | UA interprets source and styles; XHTML-only support does not imply HTML-UA status for CSS conformance. | 45 | ROLE-UA: iced source mapping does not silently claim HTML parser behavior. |
| CSS22-000061 | [§3 #property](../tmp/css22/spec/conform.html#property); Definition | CSS properties are named rendering parameters assigned through specificity, cascade and inheritance. | 51 | PROPERTY: computed property assignment is distinct from whether a box paints it. |
| CSS22-000062 | [§3 #conformance](../tmp/css22/spec/conform.html#conformance); Exception list first item | A non-rendering stylesheet-reading application must respect general points1–3. | 72 | ROLE-VALIDATOR: a parser-only result is not evidence of rendering conformance. |
| CSS22-000063 | [§3 #conformance](../tmp/css22/spec/conform.html#conformance); Exception list second item | An authoring tool is required to output valid stylesheets. | 72 | ROLE-GENERATOR: apply only if an interface emits stylesheet source, not just binary artifacts. |
| CSS22-000064 | [§3 #conformance](../tmp/css22/spec/conform.html#conformance); Exception list third item, approximation sentence | A rendering UA may approximate values when required, subject to the actual-value rules. | 50 | ACTUAL: retain specified/computed values separately from a justified device approximation. |
| CSS22-000065 | [§3 #conformance](../tmp/css22/spec/conform.html#conformance); Device inability paragraph | A particular device’s inability to implement a feature does not itself imply non-conformance. | 70 | DEVICE: dependency/API absence is not silently equated to a physical-device limitation. |
| CSS22-000066 | [§3 #conformance](../tmp/css22/spec/conform.html#conformance); User stylesheet file paragraph, exemption | The file requirement exempts devices without any means of writing or specifying files. | 71 | FILE-EXEMPTION: file-capable native targets do not receive an invented exemption. |
| CSS22-000067 | [§3 #conformance](../tmp/css22/spec/conform.html#conformance); Same paragraph final sentence | UA may additionally expose other means of user preferences, such as a GUI. | 71 | PREFERENCES: GUI controls supplement the required native file interface. |
| CSS22-000068 | [§3 #conformance](../tmp/css22/spec/conform.html#conformance); Form controls and frames paragraph | CSS22 does not specify property applicability/styling for form controls and frames; UA application is permitted. | 61 | CONTROL: record the approved product control contract instead of attributing all internal behavior to CSS22. |
| CSS22-000069 | [§3 #errors](../tmp/css22/spec/conform.html#errors); Section3.3 paragraph | Error behavior is specified throughout CSS, including the linked parsing-error rules. | 49 | ERROR-ROUTING: preserve local required handling rather than one blanket fatal-error policy. |
| CSS22-000070 | [§3 #text-css](../tmp/css22/spec/conform.html#text-css); Section3.4 paragraph | Separate Internet stylesheets are byte message entities with encoding information; text/css represents an independent CSS document under the cited MIME/HTTP registration references. | 62 | ENTITY: preserve actual metadata and representation bytes for the CSS detection algorithm. |

## Lineage, interpretation and remaining work

The source-model rows support CSS22-000008; validity/recovery rows support000004–
000007; role/device/control rows qualify the applicability of000001–000012.
These parent/child relationships are traceability, not proof that every required
child or cross-property interaction is extracted. Referenced cascade, selectors,
actual values and error algorithms still require their own detailed records.

The chapter’s HTML source/tree example and its figure are informative under the
explicit example scope. No HTML parsing behavior is imported into the iced source
language from that example. The explanatory CSS1/CSS2 compatibility statements
are preserved without claiming a full older-version semantic comparison.

The device and native-control permissions do not reduce the user's approved
product requirements. Intrinsic-dimension suppression is a normative permission,
not an adopted policy or an invented security requirement. The raster1px rule
applies only when resolution information is unreliable; what constitutes reliable
format metadata remains the resource owner’s qualification work.

The definition of preceding element is narrower than a casual reading of all
prior nodes in document order. This ledger preserves the exact defined relation;
uses of different traversal terms elsewhere require their own context review.

[Message-entity reference review](css-message-entity-audit.md) covers the retained
RFC invocation boundaries. ISO8879 and the broader document-language/reference
closure remain tracked by [reference intake](normative-reference-register.md).
No unretained external definition is claimed as reviewed here. Chapter3 source
traversal is now represented by explicit records and informative dispositions,
but full reference and interaction closure is not asserted.

Total explicit CSS ledger records after this continuation: 70. That count is not
a denominator for all CSS requirements. The full normative extraction blocker
remains open, and no planned case in this file has run against a product parser.
