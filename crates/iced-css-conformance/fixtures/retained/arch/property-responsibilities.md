# Screen property responsibility map

Status: proposed component ownership, not implementation coverage. Every property also passes through section 4 parsing/cascade/value stages and requires its applicable animation-type assessment. Shorthands expand into constituent properties.

The property set comes from [property-inventory.md](property-inventory.md); exact definition metadata is in [property-definitions.md](property-definitions.md). Component names below are architecture responsibilities, not existing crates or verified implementations. Cross-component effects are not limited to those summarized here.

| Property | Proposed responsibility | Evidence / implementation status |
| --- | --- | --- |
| [background](../tmp/css22/spec/colors.html#propdef-background) | Painting and resources; text color integration | Definition extracted; implementation mapping and validation pending |
| [background-attachment](../tmp/css22/spec/colors.html#propdef-background-attachment) | Painting and resources; text color integration | Definition extracted; implementation mapping and validation pending |
| [background-color](../tmp/css22/spec/colors.html#propdef-background-color) | Painting and resources; text color integration | Definition extracted; implementation mapping and validation pending |
| [background-image](../tmp/css22/spec/colors.html#propdef-background-image) | Painting and resources; text color integration | Definition extracted; implementation mapping and validation pending |
| [background-position](../tmp/css22/spec/colors.html#propdef-background-position) | Painting and resources; text color integration | Definition extracted; implementation mapping and validation pending |
| [background-repeat](../tmp/css22/spec/colors.html#propdef-background-repeat) | Painting and resources; text color integration | Definition extracted; implementation mapping and validation pending |
| [border](../tmp/css22/spec/box.html#propdef-border) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-bottom](../tmp/css22/spec/box.html#propdef-border-bottom) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-bottom-color](../tmp/css22/spec/box.html#propdef-border-bottom-color) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-bottom-style](../tmp/css22/spec/box.html#propdef-border-bottom-style) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-bottom-width](../tmp/css22/spec/box.html#propdef-border-bottom-width) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-collapse](../tmp/css22/spec/tables.html#propdef-border-collapse) | Table formatting and painting | Definition extracted; implementation mapping and validation pending |
| [border-color](../tmp/css22/spec/box.html#propdef-border-color) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-left](../tmp/css22/spec/box.html#propdef-border-left) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-left-color](../tmp/css22/spec/box.html#propdef-border-left-color) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-left-style](../tmp/css22/spec/box.html#propdef-border-left-style) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-left-width](../tmp/css22/spec/box.html#propdef-border-left-width) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-right](../tmp/css22/spec/box.html#propdef-border-right) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-right-color](../tmp/css22/spec/box.html#propdef-border-right-color) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-right-style](../tmp/css22/spec/box.html#propdef-border-right-style) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-right-width](../tmp/css22/spec/box.html#propdef-border-right-width) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-spacing](../tmp/css22/spec/tables.html#propdef-border-spacing) | Table formatting and painting | Definition extracted; implementation mapping and validation pending |
| [border-style](../tmp/css22/spec/box.html#propdef-border-style) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-top](../tmp/css22/spec/box.html#propdef-border-top) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-top-color](../tmp/css22/spec/box.html#propdef-border-top-color) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-top-style](../tmp/css22/spec/box.html#propdef-border-top-style) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-top-width](../tmp/css22/spec/box.html#propdef-border-top-width) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [border-width](../tmp/css22/spec/box.html#propdef-border-width) | Box geometry; paint integration | Definition extracted; implementation mapping and validation pending |
| [bottom](../tmp/css22/spec/visuren.html#propdef-bottom) | Box generation, flow, and positioning | Definition extracted; implementation mapping and validation pending |
| [caption-side](../tmp/css22/spec/tables.html#propdef-caption-side) | Table formatting and painting | Definition extracted; implementation mapping and validation pending |
| [clear](../tmp/css22/spec/visuren.html#propdef-clear) | Box generation, flow, and positioning | Definition extracted; implementation mapping and validation pending |
| [clip](../tmp/css22/spec/visufx.html#propdef-clip) | Overflow, clipping, visibility; interaction integration | Definition extracted; implementation mapping and validation pending |
| [color](../tmp/css22/spec/colors.html#propdef-color) | Painting and resources; text color integration | Definition extracted; implementation mapping and validation pending |
| [content](../tmp/css22/spec/generate.html#propdef-content) | Generated content, counters, and list markers | Definition extracted; implementation mapping and validation pending |
| [counter-increment](../tmp/css22/spec/generate.html#propdef-counter-increment) | Generated content, counters, and list markers | Definition extracted; implementation mapping and validation pending |
| [counter-reset](../tmp/css22/spec/generate.html#propdef-counter-reset) | Generated content, counters, and list markers | Definition extracted; implementation mapping and validation pending |
| [cursor](../tmp/css22/spec/ui.html#propdef-cursor) | Interaction | Definition extracted; implementation mapping and validation pending |
| [direction](../tmp/css22/spec/visuren.html#propdef-direction) | Bidirectional text; direction-sensitive layout | Definition extracted; implementation mapping and validation pending |
| [display](../tmp/css22/spec/visuren.html#propdef-display) | Box generation, flow, and positioning | Definition extracted; implementation mapping and validation pending |
| [empty-cells](../tmp/css22/spec/tables.html#propdef-empty-cells) | Table formatting and painting | Definition extracted; implementation mapping and validation pending |
| [float](../tmp/css22/spec/visuren.html#propdef-float) | Box generation, flow, and positioning | Definition extracted; implementation mapping and validation pending |
| [font](../tmp/css22/spec/fonts.html#propdef-font) | Font selection and inline text | Definition extracted; implementation mapping and validation pending |
| [font-family](../tmp/css22/spec/fonts.html#propdef-font-family) | Font selection and inline text | Definition extracted; implementation mapping and validation pending |
| [font-size](../tmp/css22/spec/fonts.html#propdef-font-size) | Font selection and inline text | Definition extracted; implementation mapping and validation pending |
| [font-style](../tmp/css22/spec/fonts.html#propdef-font-style) | Font selection and inline text | Definition extracted; implementation mapping and validation pending |
| [font-variant](../tmp/css22/spec/fonts.html#propdef-font-variant) | Font selection and inline text | Definition extracted; implementation mapping and validation pending |
| [font-weight](../tmp/css22/spec/fonts.html#propdef-font-weight) | Font selection and inline text | Definition extracted; implementation mapping and validation pending |
| [height](../tmp/css22/spec/visudet.html#propdef-height) | Box sizing and containing blocks | Definition extracted; implementation mapping and validation pending |
| [left](../tmp/css22/spec/visuren.html#propdef-left) | Box generation, flow, and positioning | Definition extracted; implementation mapping and validation pending |
| [letter-spacing](../tmp/css22/spec/text.html#propdef-letter-spacing) | Inline text; decoration painting | Definition extracted; implementation mapping and validation pending |
| [line-height](../tmp/css22/spec/visudet.html#propdef-line-height) | Inline text | Definition extracted; implementation mapping and validation pending |
| [list-style](../tmp/css22/spec/generate.html#propdef-list-style) | Generated content, counters, and list markers | Definition extracted; implementation mapping and validation pending |
| [list-style-image](../tmp/css22/spec/generate.html#propdef-list-style-image) | Generated content, counters, and list markers | Definition extracted; implementation mapping and validation pending |
| [list-style-position](../tmp/css22/spec/generate.html#propdef-list-style-position) | Generated content, counters, and list markers | Definition extracted; implementation mapping and validation pending |
| [list-style-type](../tmp/css22/spec/generate.html#propdef-list-style-type) | Generated content, counters, and list markers | Definition extracted; implementation mapping and validation pending |
| [margin](../tmp/css22/spec/box.html#propdef-margin) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [margin-bottom](../tmp/css22/spec/box.html#propdef-margin-bottom) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [margin-left](../tmp/css22/spec/box.html#propdef-margin-left) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [margin-right](../tmp/css22/spec/box.html#propdef-margin-right) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [margin-top](../tmp/css22/spec/box.html#propdef-margin-top) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [max-height](../tmp/css22/spec/visudet.html#propdef-max-height) | Box sizing and containing blocks | Definition extracted; implementation mapping and validation pending |
| [max-width](../tmp/css22/spec/visudet.html#propdef-max-width) | Box sizing and containing blocks | Definition extracted; implementation mapping and validation pending |
| [min-height](../tmp/css22/spec/visudet.html#propdef-min-height) | Box sizing and containing blocks | Definition extracted; implementation mapping and validation pending |
| [min-width](../tmp/css22/spec/visudet.html#propdef-min-width) | Box sizing and containing blocks | Definition extracted; implementation mapping and validation pending |
| [outline](../tmp/css22/spec/ui.html#propdef-outline) | Outline painting | Definition extracted; implementation mapping and validation pending |
| [outline-color](../tmp/css22/spec/ui.html#propdef-outline-color) | Outline painting | Definition extracted; implementation mapping and validation pending |
| [outline-style](../tmp/css22/spec/ui.html#propdef-outline-style) | Outline painting | Definition extracted; implementation mapping and validation pending |
| [outline-width](../tmp/css22/spec/ui.html#propdef-outline-width) | Outline painting | Definition extracted; implementation mapping and validation pending |
| [overflow](../tmp/css22/spec/visufx.html#propdef-overflow) | Overflow, clipping, visibility; interaction integration | Definition extracted; implementation mapping and validation pending |
| [padding](../tmp/css22/spec/box.html#propdef-padding) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [padding-bottom](../tmp/css22/spec/box.html#propdef-padding-bottom) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [padding-left](../tmp/css22/spec/box.html#propdef-padding-left) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [padding-right](../tmp/css22/spec/box.html#propdef-padding-right) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [padding-top](../tmp/css22/spec/box.html#propdef-padding-top) | Box geometry | Definition extracted; implementation mapping and validation pending |
| [position](../tmp/css22/spec/visuren.html#propdef-position) | Box generation, flow, and positioning | Definition extracted; implementation mapping and validation pending |
| [quotes](../tmp/css22/spec/generate.html#propdef-quotes) | Generated content, counters, and list markers | Definition extracted; implementation mapping and validation pending |
| [right](../tmp/css22/spec/visuren.html#propdef-right) | Box generation, flow, and positioning | Definition extracted; implementation mapping and validation pending |
| [table-layout](../tmp/css22/spec/tables.html#propdef-table-layout) | Table formatting and painting | Definition extracted; implementation mapping and validation pending |
| [text-align](../tmp/css22/spec/text.html#propdef-text-align) | Inline text; decoration painting | Definition extracted; implementation mapping and validation pending |
| [text-decoration](../tmp/css22/spec/text.html#propdef-text-decoration) | Inline text; decoration painting | Definition extracted; implementation mapping and validation pending |
| [text-indent](../tmp/css22/spec/text.html#propdef-text-indent) | Inline text; decoration painting | Definition extracted; implementation mapping and validation pending |
| [text-transform](../tmp/css22/spec/text.html#propdef-text-transform) | Inline text; decoration painting | Definition extracted; implementation mapping and validation pending |
| [top](../tmp/css22/spec/visuren.html#propdef-top) | Box generation, flow, and positioning | Definition extracted; implementation mapping and validation pending |
| [unicode-bidi](../tmp/css22/spec/visuren.html#propdef-unicode-bidi) | Bidirectional text; direction-sensitive layout | Definition extracted; implementation mapping and validation pending |
| [vertical-align](../tmp/css22/spec/visudet.html#propdef-vertical-align) | Inline text | Definition extracted; implementation mapping and validation pending |
| [visibility](../tmp/css22/spec/visufx.html#propdef-visibility) | Overflow, clipping, visibility; interaction integration | Definition extracted; implementation mapping and validation pending |
| [white-space](../tmp/css22/spec/text.html#propdef-white-space) | Inline text; decoration painting | Definition extracted; implementation mapping and validation pending |
| [width](../tmp/css22/spec/visudet.html#propdef-width) | Box sizing and containing blocks | Definition extracted; implementation mapping and validation pending |
| [word-spacing](../tmp/css22/spec/text.html#propdef-word-spacing) | Inline text; decoration painting | Definition extracted; implementation mapping and validation pending |
| [z-index](../tmp/css22/spec/visuren.html#propdef-z-index) | Stacking and painting order | Definition extracted; implementation mapping and validation pending |

Coverage check: exactly 90 unique screen-focused property names, each assigned once as a primary responsibility. This checks inventory coverage only. It does not account for all normative prose, selector behavior, at-rules, resource handling, or value-type definitions; those need separate requirement ledgers.
