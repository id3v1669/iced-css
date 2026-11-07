# iced-css

CSS 2.2 support for [iced](https://github.com/iced-rs/iced): write a stylesheet, capture a
tree of boxes, and get a laid-out, painted and interactive iced widget.

```rust
use iced_css::{box_tree::{BoxTree, capture_limits}, capture::adapters};

// Capture a tree. `style_id` gives each box an id for the stylesheet.
let root = adapters::column_from_vec(children, capture_limits())?
    .style_id("page", attribute_limits)?
    .erase();

// Cascade once; the tree lays out again at any size.
let tree = BoxTree::new(root, "#page { padding: 16px; background-color: #f4f1ea; }")?;

// In your view: host native iced widgets in leaf boxes and handle presses.
let element = tree
    .clone()
    .interactive()
    .widget("action", iced::widget::button("Save").on_press(Message::Save))
    .on_press("card", Message::OpenCard)
    .into();
```

`BoxTree::restyled(&[sheet, ...])` swaps in edited or additional author sheets without
capturing the tree again, which is enough to build hot reload on.

`crates/iced-css-examples` is a complete page; run it with `cargo run -p iced-css-examples`.

## What works

- **Selectors and cascade:** type, class, id, attribute and combinator selectors, including
  `:hover`, `:active` and `:focus`, which restyle the tree live.
- **Block layout:** nested boxes, margin collapsing, floats, clearance, formatting contexts,
  `min-`/`max-` sizes, percentages, `em` and `ex`.
- **Positioning:** relative, absolute and fixed, with `z-index` stacking.
- **Inline text:** styled runs in one paragraph that wraps at the box's width and flows past
  floats: size, color, weight, style, family, decoration, inline background,
  `text-align`, `white-space`, `text-transform`, `line-height`.
- **Tables:** `display: table` and its parts, with spans from `colspan`/`rowspan` attributes,
  anonymous parts, `border-spacing`, captions and cell `vertical-align`.
- **Also:** `inline-block` and `inline-table` lines, lists and markers, `:before`/`:after` with counters and
  quotes, `visibility`, outlines and every border style (collapsed tables resolve edges between cells that meet along a whole edge, and share one width per axis), `overflow` and `clip` clipping, wheel scrolling, `cursor`, `empty-cells`
  and `table-layout`.
- **Background images** (with the `image` Cargo feature): local files through `url()`, with `repeat`, `position` and
  `attachment`; relative paths resolve against `BoxTree::with_base`. Paths outside the base
  and remote URLs draw nothing.
- **Keyboard:** boxes with a press message or `tabindex` take focus by click and Tab, and
  match `:focus`; Enter and Space press them.
- **Native widgets:** any iced `Element` can sit in a leaf box and receives input; Tab moves through
  them in box order, entering the first focusable one each holds, disabled or not. `:focus` matches
  a widget's box after Tab but not after a click on it.

## Animation

`BoxTreeAnimations` runs explicit requests between two styled trees: colors (`color` reaches
what inherits or uses it, plus `background-color`, border and outline colors) and lengths
(`width`, `height`, their min and max, margins, paddings, border and outline widths, box
offsets). A style change without a request applies at once. Time comes from luminate; a
`Transition` adds an easing, cut off at the two ends.

## Known limits

Approximated or missing: text beside an `inline-block` or `inline-table` sits in the line as
one box, so lines break around whole pieces, spaces at their ends are lost and boxes align at their bottoms for `baseline` (`top` and `middle` work); `border-collapse` uses the widest cell border per axis for spacing, a cell beside a spanning one keeps its own part of the edge, and an anonymous table takes `border-collapse` from the cell it wraps;
`justify` stretches lines to the widest line rather than the box, leaves a line split at a float's end unstretched, and is start-aligned in rtl;
one line height and alignment per paragraph; text widens past
floats once per paragraph; `::before` (CSS 2.2 uses `:before`). Generated content takes `display`, `clear` and
text properties, not other box properties. `list-style-image` replaces outside markers only, and needs the `image` feature. Inline box sides take spaces across, so widths round to about a sixth of the font size, a percentage or negative margin is 0, a line can break between a side and its text, `justify` stretches them, right-to-left text keeps them in logical order, and an inline with no runs draws none; their height follows each run's font size. An rtl scroller does not scroll across. `table-layout: fixed` reads `table-column` widths but not column groups, ignores column elements past the cells, keeps a right-anchored absolute table's left edge when its columns grow it, and lets content pick collapsed-border overlaps. A row of hidden
empty cells keeps its height. `clip`ped content still extends a scroller's range. An own-text
scrolling list item's outside marker moves with the text, unclipped. Animations do not cover `auto`, transparent borders, a length to a percentage,
inline backgrounds and border colors, fonts or keyword values; those and `!important` targets apply at once. A descendant or border whose own color has the same two ends as the
animated `color` moves with it.

Not read at all: `letter-spacing` and `unicode-bidi`, the paged-media properties, and URL cursors (their keyword fallback is used). `word-spacing` adds stand-in spaces, positive values only. `text-indent` is approximated by leading spaces, positive lengths only
(no percentages or hanging indents), a word too wide beside it moves to the next line, one wider than the block wraps into blank lines, and none applies when an inline-block opens the line. `font-variant: small-caps` scales capitals, splitting combining marks from their letters and
sizing decorations by part; generated content and markers take it only from their element.

## Cost

Measured on one machine in a release build (`tests/box_tree_scale.rs`): a cascade takes about 40 ms for 2,000 boxes and grows linearly; a layout with a fake shaper takes under 1 ms there. Rules multiply with boxes, with no selector index: 1,000 rules over 2,000 boxes take about 660 ms. The first hover of a box some `:hover` rule can select cascades the whole tree once and is then cached, up to 64 states: a class rule on every card means a stall per card on a large page.

A cascade over more than 100 million box and rule pairs is refused with an error; at that size simple selectors take about half a minute on the UI thread, and selectors built to backtrack can take many minutes, since total matching work is not metered: treat stylesheets as trusted input, like code. A selector that overruns its own matching budget refuses the whole cascade: at construction that is an error, and for a later interaction the previous style stays.

## Status

Under active development and not yet published. Conformance is not measured.
