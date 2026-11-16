# Normative ledger continuation — syntax, recovery and basic values

Status: retained source reviewed; requirements specified; implementation unverified.
This bounded audit covers CSS 2.2 §4.1–§4.3 inclusive, every paragraph, token
and macro table row, core grammar production, list item and scoped example/note.
It also covers §4.4.1, independently read after checking that the existing
[encoding ledger](normative-encoding-ledger.md) covers §4.4 only before §4.4.1.
That preceding encoding-detection region is not duplicated here.

Source identity: retained [syndata.html](../tmp/css22/spec/syndata.html),
CSS22 FPWD 12 April 2016, with exact artifact identity/hash in
[source baseline](normative-source-baseline.md). Locators below use local anchors,
named paragraphs/rows/productions and retained HTML line numbers. Line numbers
supplement the anchor and prose locator; they are not identity keys.

CSS22-000071 through CSS22-000184 were allocated centrally at integration. No existing record is
retired. Syntax/recovery rows support CSS22-000004–000007 and CSS22-000048;
value/application rows support CSS22-000005–000006 and CSS22-000061.
Dependencies and interactions among rows remain necessary even where each row
has its own semantic assertion. These children do not close those broad parents.

Common fields apply to every row: source kind is CSS22; design progress is
**unresolved** for detailed algorithm/qualification, with approved roadmap owner
contracts providing direction only; evidence status is **specified**, with no
product parser, renderer, revision, executed case or target run supplied. Tests
below are **planned semantic assertions**, not test results. The common case
prefix is `SYN-` followed by the original extraction sequence number. Every row has one primary
numbered roadmap owner; 72 cooperates on verification, 66 on source coordinates,
49 on parsing, 50 on typed values, and the specific reference owners below on
external dependencies. No new policy or dependency selection is approved here.

Applicability: all managed stylesheet input in Compile, Manual and Auto, all
native targets and both backends. Value realization additionally applies when
that value participates in a supported screen formatting/resource context;
syntax and recovery still apply when a property or medium has no screen effect.
Rows expressly qualified as author/preprocessor/device roles apply only to those
roles; they do not assert that this library emits stylesheets. Device capability
or a missing dependency is not a blanket exclusion. Each row's open issues are
the common implementation/evidence gap plus any numbered issue cited below;
“reference review” is not an assertion that a target's contents were read here.

Force notation: **D** normative definition/constraint, **M** mandatory outcome,
**S** should/recommendation (retains its force), **P** permission/constrained
choice, **U** explicitly undefined behavior boundary. Informative notes/examples
are disposed separately. A single row may describe a small exhaustive lexical
family; its planned case must exercise every listed alternative and boundary.

## Core syntax and token tables

All locators in this table are under
[§4.1.1 #tokenization](../tmp/css22/spec/syndata.html#tokenization), except
where the linked §4.1 anchor is supplied. Exact regular expressions and grammar
bytes stay authoritative in the retained tables/pre block; paraphrases below do
not replace them. Case handling depends on CSS22-000108; escaping on CSS22-000110–CSS22-000118.

| ID | Precise locator | Force and independently checkable obligation | Owner | Planned semantic assertion |
| --- | --- | --- | --- | --- |
| CSS22-000071 | [#syntax](../tmp/css22/spec/syndata.html#syntax), opening paragraph, L83–87; tokenization opening paragraph | D: core grammar and forward-compatible parsing apply across CSS levels; additional level constraints may restrict validity. | 49 | Core parse success does not imply a feature/property is valid CSS22. |
| CSS22-000072 | #syntax, “These descriptions”, L89–92 | D: this syntax description is normative; source also calls Appendix G normative. Preserve conflicting appendix scope claim as issue I1. | 45 | Source classifier records both claims; no silent priority resolution. |
| CSS22-000073 | #syntax, “In this specification”, L93–94 | D: immediately before/after excludes intervening whitespace and comments. | 49 | `1px`, `1 px`, `1/**/px` remain distinguishable. |
| CSS22-000074 | Lexical paragraph, L105–108 | D: tokenization uses listed Lex-style definitions and longest matching token; octal codes refer to ISO10646. | 49 | Competing token prefixes consume longest match; tie handling remains I2. |
| CSS22-000075 | Token IDENT; macros ident/name/nmstart/nmchar/nonascii, L114,159–165 | D: IDENT has optional single leading hyphen then nmstart and zero or more nmchar; name is one or more nmchar; nonascii is outside U+0000–007F. | 49 | Cover underscore, non-ASCII, escapes, digit-leading name versus identifier, two-leading-hyphen rejection; do not split by UTF-8 byte. |
| CSS22-000076 | Token ATKEYWORD, L115 | D: at-keyword is literal @ immediately followed by ident. | 49 | `@media` versus `@ media` and `@/**/media` have different token sequences. |
| CSS22-000077 | STRING token and string/string1/string2 macros, L116,167–169 | D: quoted-string alternatives allow escaped newline/escape sequences and exclude unescaped newline, backslash and matching quote. | 49 | Both quote kinds, opposite quote literals, escapes and all nl forms yield specified token extent. |
| CSS22-000078 | BAD_STRING and badstring/1/2, L117,170–172 | D: malformed quoted prefixes follow the bad-string alternatives, including optional final backslash. | 49 | Both opening quotes at EOF and before unescaped LF/CR/FF select recovery paths; interaction CSS22-000138–CSS22-000139. |
| CSS22-000079 | BAD_URI and baduri/1/2/3, L118,176–179 | D: malformed URI alternatives cover unquoted prefix, completed string without close, and bad string. | 49 | Exercise each alternative including whitespace/escape boundaries; no assumed browser tokenizer equivalence, I3. |
| CSS22-000080 | BAD_COMMENT and badcomment/1/2, L119,173–175 | D: unfinished comment prefixes follow both listed alternatives. | 49 | Missing closing slash with and without asterisks terminates according to EOF recovery. |
| CSS22-000081 | HASH token, L120; name macro | D: HASH is # followed by name, whose start is less restricted than IDENT. | 49 | `#123` forms HASH without automatically becoming a valid ID selector or color. |
| CSS22-000082 | NUMBER and num, L121,166 | D: lexical num permits optional sign, integer/fraction form, and optional exponent. | 49 | Signed fractions/exponents tokenize by this row; <number> value acceptance separately tracked in I4. |
| CSS22-000083 | PERCENTAGE, L122 | D: percentage token joins num and %. | 49 | Whitespace/comment before % breaks token formation. |
| CSS22-000084 | DIMENSION, L123 | D: dimension token joins num and ident. | 49 | Unknown unit still forms DIMENSION; semantic length validity remains separate. |
| CSS22-000085 | URI, L124–125; U/R/L macros L182–184 | D: URI token follows either quoted or unquoted table alternative; U/R/L allow the specified literal/escaped forms. | 49 | Cover both alternatives, every U/R/L escape family, optional inner whitespace and excluded unquoted characters; I3. |
| CSS22-000086 | UNICODE-RANGE, L126–133 | D: use all eight alternatives: one-to-six ?; one-to-five hex digits plus bounded ? suffix; six hex digits; one-to-six hex range endpoints. | 49 | All alternative boundaries tokenize; token existence alone does not add a CSS22 property accepting it. |
| CSS22-000087 | CDO/CDC, L134–135 | D: literal HTML comment delimiters have dedicated tokens, subject to longest match. | 49 | Standalone delimiters versus identifier-adjacent text retain distinct tokens. |
| CSS22-000088 | Eight punctuation rows, L136–143 | D: colon, semicolon, paired braces, parentheses and brackets are distinct literal tokens. | 49 | Each literal is punctuation outside strings/escapes; nested matching uses token context. |
| CSS22-000089 | S row L144; w/nl L180–181; #whitespace L233–237 | D: S is one or more SP/TAB/CR/LF/FF, w is zero or more; nl is LF, CRLF, CR or FF. Other space-like characters are not CSS whitespace. | 49 | Each allowed code point and CRLF; U+2003/U+3000 never collapse to S. |
| CSS22-000090 | COMMENT row, L145 | D: comments follow the listed non-nesting slash-star expression. | 49 | Embedded `/*` does not increase nesting; first valid closing sequence ends token. |
| CSS22-000091 | FUNCTION row, L146 | D: FUNCTION consists of ident immediately followed by opening parenthesis. | 49 | Whitespace/comment before parenthesis changes token kind; function closer remains separate. |
| CSS22-000092 | INCLUDES/DASHMATCH rows, L147–148 | D: ~= and vertical-bar-equals form their listed selector operator tokens. | 49 | Split characters do not become one operator. |
| CSS22-000093 | DELIM row, L149–150 | D: other characters become DELIM only if unmatched by earlier definitions and neither quote kind. | 49 | Stray punctuation is retained; a lone quote enters BAD_STRING, not generic DELIM. |
| CSS22-000094 | unicode/escape macros, L163–164 | D: numeric escape is one-to-six hex digits plus optional listed terminator; simple escape excludes newline and hexadecimal digits. | 49 | Hex prefix greediness, six-digit cutoff and CRLF terminator agree with escape prose. |
| CSS22-000095 | Core syntax introduction, L194–200 | D: core grammar admits constructs beyond CSS22; parts failing level grammar are among ignored parts. | 49 | Token/core acceptance is kept distinct from level validation and recovery; I1. |
| CSS22-000096 | stylesheet/statement productions, L203–204 | D: stylesheet consists of CDO, CDC, S and statements; statement is ruleset or at-rule. | 49 | Top-level delimiters/whitespace do not create declarations or hide following rules. |
| CSS22-000097 | at-rule production, L205 | D: ATKEYWORD plus optional S/any terminates in block or semicolon with trailing S. | 49 | Nested token delimiters do not end an at-rule early; unknown keyword recovery CSS22-000136. |
| CSS22-000098 | block production, L206 | D: balanced block contains any, nested block, ATKEYWORD or semicolon forms, with listed whitespace positions. | 49 | Inner braces/functions/strings preserve outer block boundary. |
| CSS22-000099 | ruleset/declaration-list productions, L207–210 | D: core ruleset permits optional selector; declaration-list permits declarations, at-rules, semicolon recursion or empty. | 49 | Empty/core-only rules and internal at-rules are parsed before CSS22 validity checks. |
| CSS22-000100 | selector/declaration/property/value productions, L211–214 | D: selector is one-or-more any; declaration is IDENT property, colon and nonempty value; value permits any/block/ATKEYWORD forms. | 49 | Missing value enters malformed-declaration handling; token-valid values still need property validation. |
| CSS22-000101 | any production, L215–219 | D: listed simple tokens or balanced function/parenthesis/bracket forms followed by S are any; nested contents allow any/unused. | 49 | Every listed simple token and nested form is exercised; no top-level semicolon admitted as ordinary any. |
| CSS22-000102 | unused production and following paragraph, L220–226 | D: unused includes block/ATKEYWORD/semicolon/CDO/CDC plus S; reserved for error handling, not valid CSS or future extensions. | 49 | A nested unused item does not become supported value syntax merely because core parser can consume it. |
| CSS22-000103 | #comment paragraph, L228–231 | P/D: any number of COMMENT tokens may occur outside other tokens; before/within charset they disable detection. | 49 | Comments at boundaries do not splice tokens; charset interaction remains CSS22-000016–000017. |
| CSS22-000104 | Final paragraph, L238–240 | U: input that cannot be tokenized or parsed has undefined meaning in CSS22. | 49 | Apply explicit §4.2 recovery first; remaining undefined cases require policy, not an invented CSS mandate. |

## Keywords, characters, statements and recovery

| ID | Retained anchor and precise locator | Force and independently checkable obligation | Owner | Planned semantic assertion |
| --- | --- | --- | --- | --- |
| CSS22-000105 | [#keywords](../tmp/css22/spec/syndata.html#keywords), opening paragraph L246–248 | D: keywords are unquoted identifiers. | 49 | Quoted `"auto"` cannot satisfy keyword auto. |
| CSS22-000106 | [#vendor-keywords](../tmp/css22/spec/syndata.html#vendor-keywords), opening paragraph/formats L272–282 | D/S: initial hyphen/underscore property names and keywords are reserved for vendor extensions; recommended format is prefix, vendor identifier, hyphen, meaningful name. | 49 | Lexing preserves valid prefixed identifiers; registry does not infer standard support. |
| CSS22-000107 | #vendor-keywords, last two paragraphs L295–300 | P/S: unrecognized vendor properties may be ignored under recovery; conforming core parser remains usable; authors should avoid these extensions. | 49 | Unsupported extension is ignored without damaging next declaration; author recommendation is role-conditional. |
| CSS22-000108 | [#characters](../tmp/css22/spec/syndata.html#characters), first list item L329–337 | D: CSS-controlled syntax is ASCII case-insensitive; case behavior outside CSS control remains external. | 49 | Uppercase property/keyword matches lowercase; source IDs/URIs are not globally lowercased. |
| CSS22-000109 | [#value-def-identifier](../tmp/css22/spec/syndata.html#value-def-identifier), second list item L339–349 | D: unescaped identifier characters are ASCII letters/digits, U+0080 upward, hyphen and underscore; forbidden initial digit, two hyphens, or hyphen-digit; escapes also allowed. | 49 | Literal and escaped starts are distinguished without assigning escaped punctuation structural meaning. |
| CSS22-000110 | [#escaped-characters](../tmp/css22/spec/syndata.html#escaped-characters), opening text L355–360 | D: backslash is literal within comments and at immediate stylesheet EOF (DELIM). | 49 | Comment escapes do not escape closing delimiter; trailing backslash handled with context/EOF, I5. |
| CSS22-000111 | #escaped-characters, “First”, L362–366 | D: backslash-newline is removed within strings; outside strings it is a literal backslash DELIM followed by newline. | 49 | String continuation joins text; outside-string pair never joins identifiers. |
| CSS22-000112 | #escaped-characters, “Second”, L368–373 | D: simple escape removes special meaning except for hexadecimal digit, LF, CR and FF. | 49 | Escaped quote does not close string; escaped brace does not close block. |
| CSS22-000113 | #escaped-characters, preprocessor sentence L374–375 | M: stylesheet preprocessors must not remove these escapes and change stylesheet meaning. | 49 | Any future source-emitting transform preserves token/value meaning across escapes; conditional preprocessor role. |
| CSS22-000114 | #escaped-characters, “Third”, L378–385 | D/U: up to six hexadecimal digits denote ISO10646 character; code point must not be zero; literal codepoint-zero behavior is undefined. | 49 | Six-digit extent and nonzero validation are explicit; no assumed U+0000 replacement policy, I5. |
| CSS22-000115 | #escaped-characters, ordered items L386–394 | D/S: hex-following character must be separated by whitespace or a full six digits; CRLF should count as one whitespace character. | 49 | One terminator CRLF does not create unintended string character; lexical macro interaction is recorded. |
| CSS22-000116 | #escaped-characters, “In fact”, L396–399 | D: six-digit and whitespace termination may combine; only one terminating whitespace is ignored. | 49 | Two spaces after numeric escape retain the second space; six-digit escape may consume one terminator. |
| CSS22-000117 | #escaped-characters, “If the number”, L401–405 | P: out-of-Unicode-range numeric escape may be replaced by U+FFFD. | 49 | Chosen recovery is separately specified; permission is not unconditional mandated replacement. |
| CSS22-000118 | Same paragraph, display sentence L404–406 | S: if such character is displayed, show visible symbol such as missing-character glyph; linked font algorithm remains dependency. | 57 | Conditional out-of-range displayed content has explicit glyph policy, not silent disappearance. |
| CSS22-000119 | [#statements](../tmp/css22/spec/syndata.html#statements), sole paragraph L421–429 | D/P: stylesheet is list of at-rules/rule sets with allowed surrounding whitespace. | 49 | Adjacent statements retain ordering and survive legal separator whitespace. |
| CSS22-000120 | [#at-rules](../tmp/css22/spec/syndata.html#at-rules), first two paragraphs L434–442 | D: @ immediately followed by identifier begins at-rule; extent includes next semicolon or next block, whichever first. | 49 | Semicolon inside a nested token is not mistaken for statement boundary. |
| CSS22-000121 | #at-rules, third paragraph L444–448 | M: ignore @import inside block or after any non-ignored statement other than @charset/@import. | 49 | Imported request is suppressed for nested/late import; ignored preceding statement does not automatically close import window. |
| CSS22-000122 | [#block](../tmp/css22/spec/syndata.html#block), first paragraph L498–508 | D: block spans matched braces; all brackets/parentheses/braces match and may nest; matching quotes enclose strings. | 49 | Braces inside strings and escaped quotes cannot terminate block. |
| CSS22-000123 | [#rule-sets](../tmp/css22/spec/syndata.html#rule-sets), first two paragraphs L527–536 | D: rule is selector plus declaration block; zero or more declarations/at-rules inside matched braces; declarations need semicolon except last. | 49 | Empty and final-semicolon-optional blocks work; missing nonfinal separator does not fabricate two declarations. |
| CSS22-000124 | #rule-sets, selector paragraph L541–548 | D/M: selector extends to first structural opening brace and accompanies block; unparseable CSS22 selector and following block are ignored. | 49 | Invalid selector applies no declarations; quoted brace is not structural opening brace. |
| CSS22-000125 | #rule-sets, comma paragraph L550–556 | S: error anywhere in selector should cause entire statement to be ignored, even when other comma-separated parts appear valid. | 49 | Invalid group member does not independently apply valid members; retain mandatory interaction CSS22-000124. |
| CSS22-000126 | [#declaration](../tmp/css22/spec/syndata.html#declaration), first two paragraphs L592–601 | D/P: declaration is empty or property, colon, value with surrounding whitespace; same-selector declarations may be grouped with semicolons. | 49 | Empty declarations and grouped order preserve cascade semantics; arbitrary rule movement is not licensed. |
| CSS22-000127 | #declaration, property-name paragraph L628–636 | D: property name is identifier; generic values have balanced paired delimiters/strings, permitted nesting and escaped nonstring semicolons. | 49 | Unescaped semicolon ends declaration; nested/core-grammar tension I6 remains explicit. |
| CSS22-000128 | #declaration, value and ignore paragraphs L638–646 | D/M: per-property syntax and semantics restrict values; ignore invalid property name or value. | 49 | Valid general token sequence can still invalidate whole declaration while later declarations survive. |
| CSS22-000129 | [#comments](../tmp/css22/spec/syndata.html#comments), opening paragraph L673–677 | D: comments use /* and */, occur only outside other tokens, have no rendering effect, and do not nest. | 49 | Comment between tokens is ignored; comment-looking text in URI/string remains text. |
| CSS22-000130 | #comments, SGML paragraph L679–684 | D/P: CDO/CDC are permitted only at grammar-defined locations and are not CSS comment delimiters. | 49 | Text between CDO and CDC is not blanket-hidden CSS; illegal interior placements use recovery. |
| CSS22-000131 | [#ignore](../tmp/css22/spec/syndata.html#ignore), opening paragraph L690–694 | D/M: ignore means parse illegal part to find extent, then act as though absent. | 49 | Invalid construct creates no cascade/resource effect; subsequent valid construct remains. |
| CSS22-000132 | #ignore, reservation sentences L695–698 | D/M: property:value combinations and at-keywords without vendor-leading identifiers are reserved; unrecognized combinations must be ignored except those introduced by future CSS updates. | 49 | Unsupported unprefixed extension cannot gain semantics by ad hoc name matching; supplements need approval. |
| CSS22-000133 | [#parsing-errors](../tmp/css22/spec/syndata.html#parsing-errors), Unknown properties item L708–711; [#illegalvalues](../tmp/css22/spec/syndata.html#illegalvalues) L723–724,744–745 | M/P: ignore unknown-property and illegal-value declarations; future-level conformance may accept newly defined syntax. | 49 | Unsupported value preserves an earlier valid fallback; parser acceptance does not imply CSS22 definition. |
| CSS22-000134 | #parsing-errors, Malformed declarations L747–759 | M: consume malformed declaration to its end with nesting/quotes/escapes; unexpected declaration-start token starts a malformed declaration, not statement recovery. | 49 | Missing name/colon/value and unexpected prefix recover at declaration boundary, respecting nested delimiters. |
| CSS22-000135 | #parsing-errors, Malformed statements L777–782 | M: unexpected statement tokens consume through statement end while matching delimiters, quotes and escapes. | 49 | Unexpected at-keyword/closing delimiter does not accidentally promote an inner declaration to top level. |
| CSS22-000136 | #parsing-errors, Unknown at-keywords L795–800 | M: ignore invalid at-keyword through earliest containing-block end, next semicolon inclusive, or next block inclusive. | 49 | Each of three stopping boundaries leaves following sibling syntax available without leaking nested rules. |
| CSS22-000137 | #parsing-errors, “Something inside”, L825–827 | D: ignored invalid content within an at-rule does not invalidate entire at-rule. | 49 | Invalid declaration in valid @media leaves valid neighboring declarations/rules effective. |
| CSS22-000138 | [#unexpected-eof](../tmp/css22/spec/syndata.html#unexpected-eof), opening paragraph L832–835 | M: close every open construct at stylesheet EOF, including blocks, parentheses, brackets, rules, strings and comments. | 49 | EOF repair closes each nested kind in correct order; no general premature whole-sheet discard. |
| CSS22-000139 | #parsing-errors, Unexpected end of string L853–859 | M: unescaped LF/CR/FF closes string before newline, then enclosing declaration/rule is dropped. | 49 | Recovery consumes to actual declaration boundary, so next physical line is not automatically a fresh declaration. |

## Basic values and encoding-independent characters

| ID | Retained anchor and precise locator | Force and independently checkable obligation | Owner | Planned semantic assertion |
| --- | --- | --- | --- | --- |
| CSS22-000140 | [#numbers](../tmp/css22/spec/syndata.html#numbers), first paragraph L891–901 | D: integer uses decimal digits; number is integer or digits-optional dot followed by digits; optional immediate sign; -0 equals 0 and is not negative. | 50 | Signed integer/fraction accepted as appropriate, trailing-dot/hex not accepted as number; exponent value ambiguity I4. |
| CSS22-000141 | #numbers, second paragraph L904–906 | D: individual properties may impose numeric ranges, often nonnegative. | 50 | Numeric representation success does not bypass property range checks. |
| CSS22-000142 | [#length-units](../tmp/css22/spec/syndata.html#length-units), first two paragraphs L911–919 | D/P: length measures distance and is number immediately followed by unit identifier; zero may omit unit. | 50 | Unitless positive length rejected where length required; signed zero accepted under numeric equivalence. |
| CSS22-000143 | #length-units, “Some properties”, L921–925 | S: permitted negative length unsupported by implementation should convert to nearest supported value. | 50 | Keep negative-value support/approximation separate from property prohibition; interaction CSS22-000144–CSS22-000145. |
| CSS22-000144 | #length-units, “If a negative”, L927–929 | M: forbidden negative length invalidates declaration. | 50 | Negative padding-like value cannot be silently clamped into a valid declaration. |
| CSS22-000145 | #length-units, “In cases”, L931–933 | M: unsupported used length must be approximated in actual value. | 50 | Used and actual values remain separate; earlier cascade result is not rewritten. |
| CSS22-000146 | [#absrel-units](../tmp/css22/spec/syndata.html#absrel-units), paragraph/list L935–946 | D: relative and absolute unit categories; relative units refer to other length properties; em/font-size and ex/x-height. | 50 | Values retain relative basis until required resolution stage. |
| CSS22-000147 | [#em-width](../tmp/css22/spec/syndata.html#em-width), paragraph L957–964 | D/P: em uses element computed font-size, except in font-size itself where parent font-size is basis; valid for both measurement axes. | 50 | Child margin-em uses child font-size; child font-size-em uses parent size. |
| CSS22-000148 | [#ex](../tmp/css22/spec/syndata.html#ex), first two paragraphs L966–975 | D: ex uses first available font; font-size ex uses parent ex; defined even if font lacks x glyph. | 57 | Font fallback choice affects ex basis; missing x does not make unit invalid. |
| CSS22-000149 | #ex, metrics paragraph L976–982 | P/S: reliable metrics may determine x-height; lacking reliable metrics UA may use lowercase glyph; impossible/impractical measurement should use 0.5em. The o-glyph heuristic is a possible method, not required. | 57 | Exercise reliable metric, allowed glyph-derived value, and fallback branch; chosen reliability policy needs qualification. |
| CSS22-000150 | #length-units, “When specified for the root”, L1001–1004 | D: root em/ex refer to property's initial value as stated; contextual scope unresolved I7. | 50 | Planned root font-size em/ex case uses initial basis; broader root-property assertions await I7. |
| CSS22-000151 | #length-units, “Child elements”, L1006–1008 | D: children inherit computed values, not parent's specified relative values. | 51 | Inherited resolved text-indent does not rescale with child's font size; property-specific computation remains dependency. |
| CSS22-000152 | [#x39](../tmp/css22/spec/syndata.html#x39), absolute-unit paragraph/list L1024–1044 | D: absolute lengths have fixed ratios; in/cm/mm/pt/pc/px; 1in=2.54cm, 1pt=1/72in, 1pc=12pt, 1px=0.75pt. | 50 | Every unit pair agrees through canonical ratio; derived 1in=96px without equating CSS px to device pixel. |
| CSS22-000153 | #length-units, “For a CSS device”, first sentence L1048–1050 | D/P: physical measurements or reference pixel anchor absolute units. | 70 | Target policy names actual anchor and preserves all fixed inter-unit ratios. |
| CSS22-000154 | Same paragraph L1051–1056 | S: print/high-resolution devices should use physical anchor; lower-resolution/unusual-distance devices recommended pixel anchor approximating reference pixel with whole device pixels. | 70 | Screen target policy records relevant recommendation; print condition does not exclude screen parsing of physical units. |
| CSS22-000155 | [#x40](../tmp/css22/spec/syndata.html#x40), reference-pixel paragraph L1073–1079 | D: reference pixel is visual angle at 96dpi and arm's-length distance; nominal 28in yields approximately 0.0213 degrees and 1/96in. | 70 | Physical/reference geometry calculation preserves approximation versus exact unit ratios. |
| CSS22-000156 | [#percentage-units](../tmp/css22/spec/syndata.html#percentage-units), first paragraph L1121–1128 | D: percentage is number immediately followed by %. | 50 | Split/comment-separated percent does not satisfy percentage value grammar. |
| CSS22-000157 | #percentage-units, second paragraph L1130–1136 | D: each property defines percentage basis, possibly same element, ancestor or formatting context. | 51 | Width-like and font-size-like percentages resolve against their own bases, not one universal dimension. |
| CSS22-000158 | Same paragraph root sentences L1136–1140 | D: root percentage referring to inherited property value multiplies that property's initial value. | 51 | Root inherited-basis percentage works with no artificial parent element. |
| CSS22-000159 | [#uri](../tmp/css22/spec/syndata.html#uri), first paragraph L1158–1163 | D: URI includes URL/URN and is denoted with url() in property values; RFC3986 supplies URI definition. | 62 | Preserve identifier kind and source base; do not assume every URI is HTTP resource. |
| CSS22-000160 | #uri, format paragraph L1171–1176 | D: url( optional whitespace, optionally matching quoted URI, optional whitespace, close parenthesis. | 49 | Single/double/unquoted forms; mismatched closing quote invalid. |
| CSS22-000161 | #uri, unquoted paragraph L1185–1189 | M: characters such as parentheses, whitespace and quotes in unquoted URI need CSS escaping sufficient to form URI token. | 49 | Escaped punctuation remains URI payload, not token terminator. |
| CSS22-000162 | #uri, “Depending on the type”, L1191–1194 | P: URI percent escapes may be used where allowed by URI type/RFC3986. | 62 | CSS unescaping is separate from URI escaping; no blanket percent-decoding before tokenization. |
| CSS22-000163 | #uri, relative-URI paragraph L1201–1207 | P/M: relative URIs allowed; RFC3986 §5 resolution is normative; stylesheet URI supplies base, not source document URI. | 62 | Same relative resource in differently based imported sheets resolves per actual sheet base; RFC cases need reference closure. |
| CSS22-000164 | #uri, final paragraph L1226–1228 | P: handling invalid URI or unavailable/inapplicable resource may vary. | 62 | Record selected failure behavior and any property-specific constraints; no inferred fatal sheet failure. |
| CSS22-000165 | [#counter](../tmp/css22/spec/syndata.html#counter), first paragraph L1232–1241 | D: counter names are case-sensitive identifiers; counter(name[, list-style-type]) allows whitespace between tokens and defaults to decimal. | 56 | `item` and `Item` differ while function keyword case follows CSS; omitted style yields decimal. |
| CSS22-000166 | #counter, second paragraph L1243–1246 | D: counters(name, string[, list-style-type]) refers to nested counters of same name, with optional inter-token whitespace. | 56 | Separator string remains exact; distinct single/nested functions retain distinct semantics. |
| CSS22-000167 | #counter, “See Nested counters”, L1248–1253 | M: determine values by generated-content scope rules and convert to string under content-property definition. | 56 | Nested scope/value/string cases depend explicitly on generate.html extraction. |
| CSS22-000168 | #counter, final normative paragraph L1255–1259 | D: counter values may only be referred to from content in CSS22; list style none yields empty string. | 56 | Counter function is not accepted arbitrarily in other properties; none produces zero characters. |
| CSS22-000169 | [#color-units](../tmp/css22/spec/syndata.html#color-units), first paragraph L1274–1277 | D: color is keyword or numeric RGB specification. | 50 | A syntactically generic identifier is not automatically a named color. |
| CSS22-000170 | #color-units, keyword paragraph and [#TanteksColorDiagram20020613](../tmp/css22/spec/syndata.html#TanteksColorDiagram20020613), L1279–1310 | D: all 17 named colors have their expressly supplied textual hexadecimal values. | 50 | Exhaustive map: aqua 00ffff; black 000000; blue 0000ff; fuchsia ff00ff; gray 808080; green 008000; lime 00ff00; maroon 800000; navy 000080; olive 808000; orange ffa500; purple 800080; red ff0000; silver c0c0c0; teal 008080; white ffffff; yellow ffff00. |
| CSS22-000171 | #color-units, system-color paragraph L1312–1316 | P/D: environment/system color keywords also allowed, under ui.html system-color definitions. | 50 | Deprecated system colors are not rejected merely as deprecated; detailed palette remains dependency. |
| CSS22-000172 | #color-units, hexadecimal paragraph L1339–1345 | D: # immediately followed by exactly three/six hex digits; short form replicates each digit. | 50 | #fb0 gives #ffbb00; nonhex and other lengths fail CSS22 color grammar. |
| CSS22-000173 | #color-units, rgb paragraph L1347–1353 | D: rgb() has three comma-separated integers or three percentages; 255=100%=FF; surrounding numerical whitespace allowed. | 50 | Mixed integer/percentage triples fail; all-integer/all-percentage triples give equivalent channels. |
| CSS22-000174 | #color-units, sRGB paragraph L1355–1359 | D/P: numerical colors are sRGB; fidelity may vary, but color definition is objective; SRGB/COLORIMETRY references remain dependencies. | 70 | Stored CSS color meaning remains sRGB regardless of backend fidelity. |
| CSS22-000175 | #color-units, gamma paragraph L1361–1367 | P/S: UA may limit color effort to gamma correction; should combine with device gamma for effective 2.2 under specified conditions; CSS-color scope excludes independently described image color data. | 70 | Backend qualification states CSS/output gamma path and image-color separation; reference viewing conditions unresolved. |
| CSS22-000176 | #color-units, gamut paragraph L1369–1374; devices paragraph L1385–1388 | S/M/P: known out-of-device-gamut values should be clipped/mapped; channel values must enter supported device range; higher-quality gamut mapping allowed. Device gamut may differ from nominal sRGB interval. | 70 | Out-of-range authored channels are not simply invalid declarations; preserve device-aware mapping stage and policy. |
| CSS22-000177 | [#strings](../tmp/css22/spec/syndata.html#strings), first paragraph L1397–1401 | D: either quote style allowed; matching quote inside string must be escaped. | 49 | Opposite quote remains literal, matching quote escape preserves character. |
| CSS22-000178 | #strings, newline paragraph L1412–1418 | D: no direct newline inside string; numeric U+000A escape represents CSS generic newline. | 49 | Escaped A inserts LF character; raw LF invokes bad-string recovery. |
| CSS22-000179 | #strings, continuation paragraph L1420–1423 | P/D: source lines may be continued only with backslash-escaped newline; interaction CSS22-000111 removes both characters. | 49 | Continued string is identical to joined source text, distinct from numeric LF insertion. |
| CSS22-000180 | [#unsupported-values](../tmp/css22/spec/syndata.html#unsupported-values), opening paragraph L1435–1437 and fallback prose L1447–1453 | S: unsupported value should be ignored at parsing as illegal; supported later declaration can override earlier fallback, unsupported later value cannot. | 49 | Capability-specific fallback retains valid previous declaration; this is not permission to omit required CSS22 features. |
| CSS22-000181 | [#escaping](../tmp/css22/spec/syndata.html#escaping), first paragraph L1618–1623 | M: characters unrepresentable in stylesheet encoding must use escaped ISO10646 references. | 49 | Author/source-emitter role: repertoire-external characters round-trip via numeric escape. |
| CSS22-000182 | #escaping, second paragraph L1625–1629 | S: use escaping for few unrepresentable characters; when most source would be escaped, authors should use a more suitable encoding. | 49 | Conditional authoring guidance is documented, not enforced as parser rejection. |
| CSS22-000183 | #escaping, third paragraph L1631–1635 | P/M: intermediate recoders may convert repertoire escapes to encoded bytes, but must not alter escapes canceling ASCII special meaning. | 49 | Re-encoding non-ASCII escape preserves value; escaped ASCII brace/quote stays nonstructural. |
| CSS22-000184 | #escaping, fourth paragraph L1637–1639 | M: recognized encodings must map every character correctly to ISO10646, or behave equivalently. | 49 | Each claimed decoder's character mapping is qualified, beyond mandatory UTF-8 support; no blanket lossy replacement assertion. |

## Source coverage, exclusions and reference boundaries

The parenthesized numbers below retain original extraction sequence numbers
(permanent CSS22 ID = sequence number +70). The complete reviewed traversal is §4.1 opening three paragraphs (0001–0003),
§4.1.1 both tables and all twelve named productions (0004–0034), §4.1.2 and
§4.1.2.1 (0035–0037), §4.1.3 every list item and embedded escape paragraph
(0038–0048), §4.1.4–§4.1.9 (0049–0060), §4.2 every recovery item
(0061–0069), §4.3.1–§4.3.8 (0070–0110), and §4.4.1 (0111–0114).
§4.4 before §4.4.1 is delegated to the encoding ledger, not re-extracted here.

Scoped informative dispositions under the chapter 3 rule:

- §4.1.2.2 L302–323 expressly states the whole historical-prefix section is
  informative. Its 13 organization rows are not a required extension registry.
- All `class=example`, `class="illegal example"`, inline example spans and
  expressly introduced examples were read as illustrations of the adjacent
  normative rule, not additional obligations. This includes longest-match,
  keyword quoting, vendor names, escaped identifiers, import placement, nested
  blocks/selector strings, grouped declarations, every recovery before/after
  pair, unit/percentage examples, URI resolution, counters, RGB clipping,
  string continuation and unsupported-value fallback. Their scenarios may seed
  planned cases without promoting example wording to normative force.
- §4.1.3 final `li class=note` L408–415 (escaped identifiers) is informative;
  equivalent token behavior is supported independently by the normative
  escape/identifier macros. The Unicode/ISO equivalence “Note that” paragraph
  L351–353 is a reference pointer, not proof of a reviewed edition comparison.
- §4.1.7 L538–539 (future internal at-rules), §4.3.2 L1058–1071
  (anchor consequences/history), §4.3.4 L1196–1199 (comments inside URI), and
  §4.3.6 L1390–1393 (actual-gamut note) are informative notes. Nearby normative
  token/anchor/gamut rules carry the actual obligations.
- §4.3.2 two pixel figures, captions/illustrative discussion L1081–1105 and
  linked long descriptions are illustrations, not pixel reference renderings.
  The §4.3.6 color diagram's **textual named hexadecimal values**, expressly
  introduced as the values of the 17 keywords, are definition data (0100);
  rendered swatch pixels are not reference color measurements.
- §4.4.1 Greek example L1641–1644 and HTML4 `div class=note` L1646–1666
  are informative. The HTML source-language recommendation does not establish
  an iced HTML parsing requirement.
- Navigation, contents and presentation markup contain no added CSS obligations.
  Explanatory “for example” clauses illustrate the row's defined rule; a plain
  “Note that” that restates a property-specific constraint (e.g. numeric ranges)
  is not used to evade the independently required property validation.

Reference/dependency intake follows the existing
[reference register](normative-reference-register.md),
[reference issues](normative-reference-issues.md), and
[Unicode support audit](unicode-support-audit.md). This task read retained
syndata.html plus the explicitly bounded grammar/font-size passages in I1/I7;
a citation here does not assert exhaustive target review:

| Referenced source | Required relationship and accountable roadmap owner | Remaining review |
| --- | --- | --- |
| ISO10646 and UNICODE | Character repertoire, octal/numeric escape meaning and decoder mapping; 49 with 57 | Bind exact editions and adopted Unicode policy; surrogate/noncharacter/invalid codepoint behavior must not be guessed. |
| Appendix G grammar.html | Core versus level grammar relationship; 49 with 45 | Resolve authority conflict I1 and independently extract/qualify any adopted syntax constraints. |
| selector.html and source language | Selector validity and external case rules; 49 with 47 | Detailed selector/source-name rules not derived from syntax token acceptance. |
| cascade.html #at-import/#usedValue/#actual-value/#computed-value and about.html #initial-value | Import ordering, computation, approximation and root bases; 49/50/51 respectively | Complete cross-section interaction records, especially I7. |
| fonts.html #propdef-font-size/#algorithm | Parent/first available font, ex and missing glyph behavior; 57 | Font algorithm/metrics reliability and glyph fallback evidence. |
| RFC3986, expressly §5 | URI structure, percent escapes and base resolution; 62 | Link existing retained RFC audit or complete target clauses; no independent URI algorithm claim from this audit. |
| generate.html #scope/#propdef-content and counter properties | Counter scopes and string conversion; 56 | Full nested/reset/increment/style rules and content restrictions. |
| ui.html #system-colors | Additional normative color names; 50 | Detailed system-color resolution, environment capture and user policy. |
| SRGB and COLORIMETRY | Color definition, gamut and viewing conditions; 70 | Edition/access/technical color mapping qualification; no assumed modern color API equivalence. |
| HTML4 | Informative SGML hiding and character-reference comparison; 47/45 | No normative iced HTML mapping imported from these references. |

## Open source issues and verification gaps

I1. §4.1 L89–92 calls Appendix G's grammar normative; the retained appendix is
explicitly scoped non-normative in its opening paragraph,
[grammar.html](../tmp/css22/spec/grammar.html) L40, independently checked here.
This file preserves both claims. Resolving authority requires recorded source
comparison, not treating a convenient parser grammar as automatically binding.

I2. The lexical paragraph expressly says longest match but does not independently
spell out equal-length tie-breaking or Lex case configuration. The table order,
ASCII case rule and known token overlaps need explicit algorithm review; claiming
that a library's default token priority matches this source would be unverified.

I3. URI and BAD_URI unquoted character classes differ in their literal retained
expressions. This audit deliberately references the exact bytes instead of
normalizing them into one modern URL grammar. Test every class boundary and
longest-match interaction before selecting a tokenizer implementation.

I4. §4.1.1 `num` L166 expressly permits an exponent, whereas §4.3.1 number prose
L897–901 enumerates only integer/fraction decimal forms. Token recognition and
property-value validity are distinct, so preserving exponent tokens without
claiming exponent <number> validity is a viable interpretation to evaluate.
Recommended interpretation for owner 49: retain the broader lexical NUMBER
recognition, then validate each property against its value grammar, as required
by §4.1.8 and the existing §3.2 ledger. This separates the two explicit source
stages without adding authored <number> forms. It remains a proposed detailed
interpretation, not an approved resolution or verified parser behavior.

I5. Numeric escape zero prohibition and literal U+0000 undefined behavior do not
supply a full invalid-scalar algorithm. Out-of-range replacement is a permission,
not a universal requirement. Immediate-EOF backslash wording also needs checking
against quoted-string BAD_STRING and mandatory construct closure. No modern CSS
preprocessing/replacement convention is silently adopted.

I6. Declaration prose requires escaping semicolons outside strings, while core
block/unused productions can consume semicolons in nested constructs for parsing
and recovery. Core consumption must not be confused with accepted property value
syntax; precise normative/property boundaries need qualification.

I7. §4.3.2 L1001–1004 broadly says root em/ex refer to the property's initial
value; preceding em/ex paragraphs provide explicit font-size exceptions. A
contextual reading for root font-size would avoid overriding root margin's
computed-font-size basis, but the linked font-size/initial-value sections must be
reviewed before adopting that reading. Additional bounded reading of
[fonts.html #propdef-font-size](../tmp/css22/spec/fonts.html#propdef-font-size),
L675–688 and L767–769, confirms the initial medium/absolute-length computed
value and explicitly assigns em/ex on all other properties to the current
element, while font-size uses the parent. This supports recommending the
root-initial exception only for font-size resolution. The prose also loosely
describes ex in terms of computed font size; the precise x-height definition
in §4.3.2 must remain operative. This is proposed reconciliation, not a new
font chapter extraction or an approved test oracle for every root property.

I8. Recommendations/permissions for negative unsupported lengths, unknown
resources, color fidelity/gamut and unsupported values preserve their conditions.
They require concrete product policy and do not remove mandatory property,
parser, source or backend evidence obligations. Color gamut/transfer decisions
also depend on retained external definitions and target capability evidence.

All 114 records are source extraction with planned checks. They do not
claim 114 implemented tests, coverage of every CSS chapter, complete reference
closure, successful backend runs, or product conformance. No production files,
dependency selection, tests, commits or pushes are part of this artifact.

## Q277 interpretation update

[Grammar reconciliation](grammar-authority-reconciliation.md) supersedes the
earlier unresolved architecture choice in I1: chapter4's explicit token/character
and recovery rules take priority, with AppendixG a compatible grammar aid. The
source contradiction is still disclosed. I4's separation between broad lexical
recognition and decimal property-value validation is selected. I2 tie-breaking,
I3 URI boundaries and I5 invalid-scalar/EOF work remain open. Q271 already
resolved I7's root font-size interpretation in the font ledger.

## Q278 scanner continuation

[Scanner qualification](scanner-contract-qualification.md) records a retained Lex
priority reference, a selected explicit tie policy and five passing Rust probe
tests. I2 now has a priority interpretation; I3 has measured URI/BAD_URI overlaps
and prefix boundaries. Whole-parser recovery and invalid-scalar/EOF integration
remain open; earlier unqualified status statements are historical.

## Q279 scalar continuation

[CSS scalar boundary](css-scalar-boundary-review.md) selects stage-specific handling
for literal NUL, zero/surrogate/out-of-range numeric escapes and noncharacters,
with retained candidate Unicode17 definitions. I5 remains open for EOF context
and execution of the integrated decoder/parser cases.

## Q280 BAD_URI exception

[The longer-match regression](bad-uri-continuation-reconciliation.md) demonstrates
that I3 also requires a character-class reconciliation. Select AppendixG's narrower
baduri1 class as the explicit exception to Q277, retaining the other chapter4
macros. Q278's tie priority alone is not sufficient. Six bounded probe tests pass;
whole-parser recovery and EOF integration remain open.

## Q285 EOF/string interpretation

The [EOF reconciliation](eof-string-reconciliation.md) selects logical closure
with a final literal string backslash and preserves newline-error recovery.
Twenty-two bounded Rust assertions pass. This settles the named I5 interpretation;
whole-stylesheet recovery, URI EOF and diagnostic integration remain unqualified.

## Q286 declaration/recovery boundary

[Declaration reconciliation](declaration-recovery-reconciliation.md) distinguishes
accepted property values from nested tokens consumed only for recovery. Specific
URI token syntax retains internal semicolons as URI data. This selects the named
I6 interpretation; whole-parser recovery cases remain unrun.
