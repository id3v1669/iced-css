# Architecture section 67 — CSS 2.2 property animation registry

Status: approved under standing delegated design authority (Q250).
All 90 screen properties have an entry below. This is a semantic design registry,
not 90 implemented animation tests or an expansion of authored CSS syntax.

## Combination dispatch

B = by computed value under retained Web Animations Appendix A, using each CSS22
computed representation below. Corresponding components interpolate/add/accumulate
by their value type; incompatible component types/counts or unequal discrete components
use D for the entire property. Do not assume repeatable-list behavior without a source
assigning it. The table uses the approved fixed reference set, not unretained newer
property modules. CSS22 syntax remains controlling.

D = discrete midpoint interpolation (start below 0.5, end otherwise), not additive.
Use Values4's non-additive fallback (retain the first operand); accumulation uses the
specified type procedure, otherwise addition. N = not animatable: no property effect;
report request ineligibility rather than silently substituting D. S = expand the CSS22
shorthand and dispatch to constituent entries, preserving their independent eligibility.
V = approved visibility exception: an endpoint visible remains visible for interior
progress; outside choose the nearer endpoint, and neither-visible uses D.
FW = legacy weight number interpolation rounded to nearest hundred, ties toward positive
infinity; addition sums, accumulation follows addition, final property range applies.
FS = normal becomes internal oblique 0 degrees, oblique without angle becomes 14 degrees
under the selected Fonts4 rule; compatible oblique angles combine numerically. Italic
is a distinct keyword and incompatible pairs use D. Authored angles remain prohibited.

Numbers, dimensions and percentages interpolate as weighted sums; addition sums compatible
components. Integer interpolation rounds nearest with halfway toward positive infinity.
Mixed length/percentage retains separate components when unresolved. Final combination
range checking uses the target context at computed/used stage; do not clamp intermediate
components early. Keywords auto/normal/none/invert are typed alternatives, not invented
numeric endpoints. Resolved colors use the approved gamma-encoded sRGB premultiplied-alpha
path, with zero-alpha inverse handling. Border widths preserve style-mandated zero and
important/suppression ordering. font-size preserves keyword resolution and its f32 domain.

Sources: [Web Animations types](../tmp/web-animations-1/index.html#animating-properties),
[legacy/default exceptions](../tmp/web-animations-1/index.html#animation-types),
[Values combination](architecture-20-numeric-animation.md),
[color bridge](architecture-21-color-animation.md), [fixed module audit](animation-registry-audit.md).
The visibility conflict is resolved by Q068. Display3 supplies N for display; Fonts4
supplies D for font-family/font-variant and FS for font-style. Other missing CSS22
animation labels use Appendix A's B default, not a guess from modern browser behavior.

## Complete per-property entries

Each entry's definition link supplies validity/range/computed-value details. Test ID
ANIM-<property> requires compatible endpoints, incompatible alternatives, midpoint,
extrapolation/final range, addition/accumulation or ineligibility, and dependency refresh.
Shorthands require longhand expansion/reset tests. A conservative full style/layout/paint
invalidation is selected for every changed presentation; later optimization may use the
primary owner after equivalence testing. This avoids missing cross-property effects.

| Property/source | Rule | CSS22 computed representation | Combination/invalidation owner / test |
| --- | --- | --- | --- |
| [background](../tmp/css22/spec/colors.html#propdef-background) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-background |
| [background-attachment](../tmp/css22/spec/colors.html#propdef-background-attachment) | B | as specified | CSS semantic registry + full invalidation; ANIM-background-attachment |
| [background-color](../tmp/css22/spec/colors.html#propdef-background-color) | B | as specified | CSS semantic registry + full invalidation; ANIM-background-color |
| [background-image](../tmp/css22/spec/colors.html#propdef-background-image) | B | absolute URI or none | CSS semantic registry + full invalidation; ANIM-background-image |
| [background-position](../tmp/css22/spec/colors.html#propdef-background-position) | B | for &lt;length&gt; the absolute value, otherwise a percentage | CSS semantic registry + full invalidation; ANIM-background-position |
| [background-repeat](../tmp/css22/spec/colors.html#propdef-background-repeat) | B | as specified | CSS semantic registry + full invalidation; ANIM-background-repeat |
| [border](../tmp/css22/spec/box.html#propdef-border) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-border |
| [border-bottom](../tmp/css22/spec/box.html#propdef-border-bottom) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-border-bottom |
| [border-bottom-color](../tmp/css22/spec/box.html#propdef-border-bottom-color) | B | when taken from the 'color' property, the computed value of 'color'; otherwise, as specified | CSS semantic registry + full invalidation; ANIM-border-bottom-color |
| [border-bottom-style](../tmp/css22/spec/box.html#propdef-border-bottom-style) | B | as specified | CSS semantic registry + full invalidation; ANIM-border-bottom-style |
| [border-bottom-width](../tmp/css22/spec/box.html#propdef-border-bottom-width) | B | absolute length; '0' if the border style is 'none' or 'hidden' | CSS semantic registry + full invalidation; ANIM-border-bottom-width |
| [border-collapse](../tmp/css22/spec/tables.html#propdef-border-collapse) | B | as specified | CSS semantic registry + full invalidation; ANIM-border-collapse |
| [border-color](../tmp/css22/spec/box.html#propdef-border-color) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-border-color |
| [border-left](../tmp/css22/spec/box.html#propdef-border-left) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-border-left |
| [border-left-color](../tmp/css22/spec/box.html#propdef-border-left-color) | B | when taken from the 'color' property, the computed value of 'color'; otherwise, as specified | CSS semantic registry + full invalidation; ANIM-border-left-color |
| [border-left-style](../tmp/css22/spec/box.html#propdef-border-left-style) | B | as specified | CSS semantic registry + full invalidation; ANIM-border-left-style |
| [border-left-width](../tmp/css22/spec/box.html#propdef-border-left-width) | B | absolute length; '0' if the border style is 'none' or 'hidden' | CSS semantic registry + full invalidation; ANIM-border-left-width |
| [border-right](../tmp/css22/spec/box.html#propdef-border-right) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-border-right |
| [border-right-color](../tmp/css22/spec/box.html#propdef-border-right-color) | B | when taken from the 'color' property, the computed value of 'color'; otherwise, as specified | CSS semantic registry + full invalidation; ANIM-border-right-color |
| [border-right-style](../tmp/css22/spec/box.html#propdef-border-right-style) | B | as specified | CSS semantic registry + full invalidation; ANIM-border-right-style |
| [border-right-width](../tmp/css22/spec/box.html#propdef-border-right-width) | B | absolute length; '0' if the border style is 'none' or 'hidden' | CSS semantic registry + full invalidation; ANIM-border-right-width |
| [border-spacing](../tmp/css22/spec/tables.html#propdef-border-spacing) | B | two absolute lengths | CSS semantic registry + full invalidation; ANIM-border-spacing |
| [border-style](../tmp/css22/spec/box.html#propdef-border-style) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-border-style |
| [border-top](../tmp/css22/spec/box.html#propdef-border-top) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-border-top |
| [border-top-color](../tmp/css22/spec/box.html#propdef-border-top-color) | B | when taken from the 'color' property, the computed value of 'color'; otherwise, as specified | CSS semantic registry + full invalidation; ANIM-border-top-color |
| [border-top-style](../tmp/css22/spec/box.html#propdef-border-top-style) | B | as specified | CSS semantic registry + full invalidation; ANIM-border-top-style |
| [border-top-width](../tmp/css22/spec/box.html#propdef-border-top-width) | B | absolute length; '0' if the border style is 'none' or 'hidden' | CSS semantic registry + full invalidation; ANIM-border-top-width |
| [border-width](../tmp/css22/spec/box.html#propdef-border-width) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-border-width |
| [bottom](../tmp/css22/spec/visuren.html#propdef-bottom) | B | if specified as a length, the corresponding absolute length; if specified as a percentage, the specified value; otherwise, 'auto'. | CSS semantic registry + full invalidation; ANIM-bottom |
| [caption-side](../tmp/css22/spec/tables.html#propdef-caption-side) | B | as specified | CSS semantic registry + full invalidation; ANIM-caption-side |
| [clear](../tmp/css22/spec/visuren.html#propdef-clear) | B | as specified | CSS semantic registry + full invalidation; ANIM-clear |
| [clip](../tmp/css22/spec/visufx.html#propdef-clip) | B | 'auto' if specified as 'auto', otherwise a rectangle with four values, each of which is 'auto' if specified as 'auto' and the computed length otherwise | CSS semantic registry + full invalidation; ANIM-clip |
| [color](../tmp/css22/spec/colors.html#propdef-color) | B | as specified | CSS semantic registry + full invalidation; ANIM-color |
| [content](../tmp/css22/spec/generate.html#propdef-content) | B | On elements, always computes to 'normal'. On :before and :after, if 'normal' is specified, computes to 'none'. Otherwise, for URI values, the absolute URI; for attr() values, the resulting string; for other keywords, as specified. | CSS semantic registry + full invalidation; ANIM-content |
| [counter-increment](../tmp/css22/spec/generate.html#propdef-counter-increment) | B | as specified | CSS semantic registry + full invalidation; ANIM-counter-increment |
| [counter-reset](../tmp/css22/spec/generate.html#propdef-counter-reset) | B | as specified | CSS semantic registry + full invalidation; ANIM-counter-reset |
| [cursor](../tmp/css22/spec/ui.html#propdef-cursor) | B | as specified, except with any relative URLs converted to absolute | CSS semantic registry + full invalidation; ANIM-cursor |
| [direction](../tmp/css22/spec/visuren.html#propdef-direction) | B | as specified | CSS semantic registry + full invalidation; ANIM-direction |
| [display](../tmp/css22/spec/visuren.html#propdef-display) | N | see text | CSS semantic registry + full invalidation; ANIM-display |
| [empty-cells](../tmp/css22/spec/tables.html#propdef-empty-cells) | B | as specified | CSS semantic registry + full invalidation; ANIM-empty-cells |
| [float](../tmp/css22/spec/visuren.html#propdef-float) | B | as specified | CSS semantic registry + full invalidation; ANIM-float |
| [font](../tmp/css22/spec/fonts.html#propdef-font) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-font |
| [font-family](../tmp/css22/spec/fonts.html#propdef-font-family) | D | as specified | CSS semantic registry + full invalidation; ANIM-font-family |
| [font-size](../tmp/css22/spec/fonts.html#propdef-font-size) | B | absolute length | CSS semantic registry + full invalidation; ANIM-font-size |
| [font-style](../tmp/css22/spec/fonts.html#propdef-font-style) | FS | as specified | CSS semantic registry + full invalidation; ANIM-font-style |
| [font-variant](../tmp/css22/spec/fonts.html#propdef-font-variant) | D | as specified | CSS semantic registry + full invalidation; ANIM-font-variant |
| [font-weight](../tmp/css22/spec/fonts.html#propdef-font-weight) | FW | see text | CSS semantic registry + full invalidation; ANIM-font-weight |
| [height](../tmp/css22/spec/visudet.html#propdef-height) | B | the percentage or 'auto' (as specified) or the absolute length | CSS semantic registry + full invalidation; ANIM-height |
| [left](../tmp/css22/spec/visuren.html#propdef-left) | B | if specified as a length, the corresponding absolute length; if specified as a percentage, the specified value; otherwise, 'auto'. | CSS semantic registry + full invalidation; ANIM-left |
| [letter-spacing](../tmp/css22/spec/text.html#propdef-letter-spacing) | B | 'normal' or absolute length | CSS semantic registry + full invalidation; ANIM-letter-spacing |
| [line-height](../tmp/css22/spec/visudet.html#propdef-line-height) | B | for &lt;length&gt; and &lt;percentage&gt; the absolute value; otherwise as specified | CSS semantic registry + full invalidation; ANIM-line-height |
| [list-style](../tmp/css22/spec/generate.html#propdef-list-style) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-list-style |
| [list-style-image](../tmp/css22/spec/generate.html#propdef-list-style-image) | B | absolute URI or 'none' | CSS semantic registry + full invalidation; ANIM-list-style-image |
| [list-style-position](../tmp/css22/spec/generate.html#propdef-list-style-position) | B | as specified | CSS semantic registry + full invalidation; ANIM-list-style-position |
| [list-style-type](../tmp/css22/spec/generate.html#propdef-list-style-type) | B | as specified | CSS semantic registry + full invalidation; ANIM-list-style-type |
| [margin](../tmp/css22/spec/box.html#propdef-margin) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-margin |
| [margin-bottom](../tmp/css22/spec/box.html#propdef-margin-bottom) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-margin-bottom |
| [margin-left](../tmp/css22/spec/box.html#propdef-margin-left) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-margin-left |
| [margin-right](../tmp/css22/spec/box.html#propdef-margin-right) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-margin-right |
| [margin-top](../tmp/css22/spec/box.html#propdef-margin-top) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-margin-top |
| [max-height](../tmp/css22/spec/visudet.html#propdef-max-height) | B | the percentage as specified or the absolute length or 'none' | CSS semantic registry + full invalidation; ANIM-max-height |
| [max-width](../tmp/css22/spec/visudet.html#propdef-max-width) | B | the percentage as specified or the absolute length or 'none' | CSS semantic registry + full invalidation; ANIM-max-width |
| [min-height](../tmp/css22/spec/visudet.html#propdef-min-height) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-min-height |
| [min-width](../tmp/css22/spec/visudet.html#propdef-min-width) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-min-width |
| [outline](../tmp/css22/spec/ui.html#propdef-outline) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-outline |
| [outline-color](../tmp/css22/spec/ui.html#propdef-outline-color) | B | as specified | CSS semantic registry + full invalidation; ANIM-outline-color |
| [outline-style](../tmp/css22/spec/ui.html#propdef-outline-style) | B | as specified | CSS semantic registry + full invalidation; ANIM-outline-style |
| [outline-width](../tmp/css22/spec/ui.html#propdef-outline-width) | B | absolute length; '0' if the outline style is 'none' | CSS semantic registry + full invalidation; ANIM-outline-width |
| [overflow](../tmp/css22/spec/visufx.html#propdef-overflow) | B | as specified | CSS semantic registry + full invalidation; ANIM-overflow |
| [padding](../tmp/css22/spec/box.html#propdef-padding) | S | see individual properties | CSS semantic registry + full invalidation; ANIM-padding |
| [padding-bottom](../tmp/css22/spec/box.html#propdef-padding-bottom) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-padding-bottom |
| [padding-left](../tmp/css22/spec/box.html#propdef-padding-left) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-padding-left |
| [padding-right](../tmp/css22/spec/box.html#propdef-padding-right) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-padding-right |
| [padding-top](../tmp/css22/spec/box.html#propdef-padding-top) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-padding-top |
| [position](../tmp/css22/spec/visuren.html#propdef-position) | B | as specified | CSS semantic registry + full invalidation; ANIM-position |
| [quotes](../tmp/css22/spec/generate.html#propdef-quotes) | B | as specified | CSS semantic registry + full invalidation; ANIM-quotes |
| [right](../tmp/css22/spec/visuren.html#propdef-right) | B | if specified as a length, the corresponding absolute length; if specified as a percentage, the specified value; otherwise, 'auto'. | CSS semantic registry + full invalidation; ANIM-right |
| [table-layout](../tmp/css22/spec/tables.html#propdef-table-layout) | B | as specified | CSS semantic registry + full invalidation; ANIM-table-layout |
| [text-align](../tmp/css22/spec/text.html#propdef-text-align) | B | the initial value or as specified | CSS semantic registry + full invalidation; ANIM-text-align |
| [text-decoration](../tmp/css22/spec/text.html#propdef-text-decoration) | B | as specified | CSS semantic registry + full invalidation; ANIM-text-decoration |
| [text-indent](../tmp/css22/spec/text.html#propdef-text-indent) | B | the percentage as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-text-indent |
| [text-transform](../tmp/css22/spec/text.html#propdef-text-transform) | B | as specified | CSS semantic registry + full invalidation; ANIM-text-transform |
| [top](../tmp/css22/spec/visuren.html#propdef-top) | B | if specified as a length, the corresponding absolute length; if specified as a percentage, the specified value; otherwise, 'auto'. | CSS semantic registry + full invalidation; ANIM-top |
| [unicode-bidi](../tmp/css22/spec/visuren.html#propdef-unicode-bidi) | B | as specified | CSS semantic registry + full invalidation; ANIM-unicode-bidi |
| [vertical-align](../tmp/css22/spec/visudet.html#propdef-vertical-align) | B | for &lt;percentage&gt; and &lt;length&gt; the absolute length, otherwise as specified | CSS semantic registry + full invalidation; ANIM-vertical-align |
| [visibility](../tmp/css22/spec/visufx.html#propdef-visibility) | V | as specified | CSS semantic registry + full invalidation; ANIM-visibility |
| [white-space](../tmp/css22/spec/text.html#propdef-white-space) | B | as specified | CSS semantic registry + full invalidation; ANIM-white-space |
| [width](../tmp/css22/spec/visudet.html#propdef-width) | B | the percentage or 'auto' as specified or the absolute length | CSS semantic registry + full invalidation; ANIM-width |
| [word-spacing](../tmp/css22/spec/text.html#propdef-word-spacing) | B | for 'normal' the value '0'; otherwise the absolute length | CSS semantic registry + full invalidation; ANIM-word-spacing |
| [z-index](../tmp/css22/spec/visuren.html#propdef-z-index) | B | as specified | CSS semantic registry + full invalidation; ANIM-z-index |

## Bridge and qualification

Store CSS-owned endpoints/target bindings and use one f32 progress track per accepted
source/property/effect owner. Preserve Q225 checked private-engine addresses and Q226
request identity. A luminate type fitting four components does not prove CSS equivalence;
direct typed tracks are optional only after the same semantic tests pass. Start commands
are explicit Messages, not reevaluation side effects. Native/generic image playback is
separate from this registry and follows section 68.

The table is checked against the 90-property inventory, with no missing/duplicate entries.
No property animation product suite is reported passing. This registry settles dispatch,
combination sources, invalidation and named cases; section 72 implements and reports them.
