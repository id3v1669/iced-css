# iced-css

CSS 2.2 support for [iced](https://github.com/iced-rs/iced): write a stylesheet, capture a
tree of boxes, and get a laid-out, painted and interactive iced widget.

```rust
use iced_css::{box_tree::{BoxTree, attribute_limits, capture_limits}, capture::adapters};

// Capture a tree. `style_id` gives each box an id for the stylesheet.
let root = adapters::column_from_vec(children, capture_limits())?
    .style_id("page", attribute_limits())?
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

The same example compiles and runs as a doctest in `crates/iced-css/src/box_tree.rs`.

`BoxTree::restyled(&[sheet, ...])` swaps in edited or additional author sheets without
capturing the tree again, which is enough to build hot reload on. A sheet with an error
returns `Err` and leaves the current tree as it was, so a reload keeps the old tree rather
than unwrapping:

```rust
// On a file-watcher event or a key press:
if let Ok(sheet) = std::fs::read_to_string("page.css") {
    match app.tree.restyled(&[&sheet]) {
        Ok(next) => app.tree = next,
        Err(error) => eprintln!("stylesheet not reloaded: {error}"),
    }
}
```

`crates/iced-css-examples` is a complete page; run it with `cargo run -p iced-css-examples`.

## Building

The workspace does not build from a bare clone yet. Animation time comes from
[luminate](https://github.com/sonata-ltd/luminate)'s `animate` crate (`iced_animate`), which
`crates/iced-css/Cargo.toml` takes as a path dependency on a local checkout at
`/home/user/tmpmy/luminate/animate`, and every cargo command in the workspace reads that
manifest. Before building, clone luminate and check out the revision the workspace is
developed against:

```sh
git clone https://github.com/sonata-ltd/luminate /home/user/tmpmy/luminate
git -C /home/user/tmpmy/luminate checkout f17ce093ebe6fd54f540d66915683f78e0942a0e
```

To keep the checkout elsewhere, change the `iced_animate` path in
`crates/iced-css/Cargo.toml` and the `ANIMATE` constant in
`crates/iced-css-conformance/src/dependency.rs`, which checks the dependency against the
same path.

A plain `cargo test` leaves out the tests of the optional `image` and `svg` features: image
painting and list-style images in `crates/iced-css/tests/box_tree_render.rs`, and the
`tests/capture_image.rs` and `tests/capture_svg.rs` adapters, among others. Run them with:

```sh
cargo test --workspace --features iced-css/image,iced-css/svg
```

## What works

- **Selectors and cascade:** type, class, id, attribute and combinator selectors, including
  `:hover`, `:active` and `:focus`, which restyle the tree live.
- **Block layout:** nested boxes, margin collapsing, floats, clearance, formatting contexts,
  `min-`/`max-` sizes, percentages, `em` and `ex`.
- **Positioning:** relative, absolute and fixed, with `z-index` stacking, on boxes; an inline
  element ignores `position: relative`: its offsets and `z-index` do nothing, and it is no
  containing block for absolute descendants, which take the nearest positioned box instead.
- **Inline text:** styled runs in one paragraph that wraps at the box's width and flows past
  floats: size, color, weight, style, family, decoration, inline background,
  `text-align`, `white-space`, `text-transform`, `line-height`.
- **Tables:** `display: table` and its parts, with spans from `colspan`/`rowspan` attributes,
  anonymous parts, `border-spacing`, captions and cell `vertical-align`.
- **Also:** `inline-block` and `inline-table` lines, lists and markers, `:before`/`:after` (also with `::`) with counters and
  quotes, `visibility`, outlines and every border style (collapsed tables resolve each stretch of edge between the cells that meet there, where lines cross the later cell's border shows, and each grid line takes its widest border; an inline element's outline goes around each of its pieces on a line), `overflow` and `clip` clipping, wheel scrolling, `cursor`, `empty-cells`
  and `table-layout`.
- **Background images** (with the `image` Cargo feature): local files through `url()`, with `repeat`, `position` and
  `attachment`; relative URLs resolve against `BoxTree::with_base`, their `%` escapes decoded
  and their query and fragment dropped. Paths outside the base, also through a symlink, and
  remote URLs draw nothing.
- **Keyboard:** boxes with a press message or `tabindex` take focus by click and Tab, and
  match `:focus`; Enter and Space press them (a held Space once). With Ctrl, Alt or Logo held
  these keys are left to the app.
- **Touch:** a finger press and lift act as a left click does (`on_press`, `:active`, `:focus`);
  dragging a finger does not scroll an `overflow` box.
- **Native widgets:** any iced `Element` can sit in a leaf box and receives input; Tab moves through
  them in box order, and through the focusables each holds before leaving it, entering at the first
  (with Shift, the last), disabled or not. `:focus` matches
  a widget's box after Tab but not after a click on it.

## Animation

`BoxTreeAnimations` runs explicit requests between two styled trees: colors (`color` reaches
what inherits or uses it, plus `background-color`, border and outline colors) and lengths
(`width`, `height`, their min and max, margins, paddings, border and outline widths, box
offsets, an inline element's margins across and paddings, its side spaces growing from or
shrinking to none, a length to or from a percentage mixing them as `calc()` does, which acts as a
percentage does where its basis is not known, so a height in an auto-height block or a
width in a shrink-to-fit one applies at once), `visibility`, shown all the way through while either end is shown, with
what inherits it, and `font-size`, with the text and boxes whose sizes keep the same share
of it at both ends, as inherited, `em` and percentage sizes do, and their lengths given in
`em` or `ex` (`text-indent` and an inline's margins, padding and borders too); a pixel
`line-height` follows when it is the same share of the size at both ends, as `em` gives,
since it inherits as pixels. A `font-size` of 0 at either end applies at once. An inline's outline and offsets in `em`, and `calc()`-like
mixes of `em` and a percentage, take their end value at once; `word-spacing` scales with
the text. A
style change without a request applies at once. Time comes from luminate; a
`Transition` adds an easing, cut off at the two ends.

## Known limits

Approximated or missing: text beside an `inline-block` or `inline-table` sits in the line as
one box, so lines break around whole pieces, spaces at their ends are lost and those on the `baseline` share it, an inline-block's its last line's (its bottom margin edge with none, or when it clips) and an inline-table's its first row's, placed from the ascent and descent of the faces that draw the line, picked as the renderer's text system picks them, each character from the first face that has it (`BoxTree::layout_with` lays out with a renderer's text outside the widget as it does; `BoxTree::layout_text` has no fonts and lines bottoms up instead); `top` and `text-top` take the line box's top, `bottom` and `text-bottom` its bottom and `middle` its centre, other values take the baseline, and `vertical-align` on plain inline text is not applied; `border-collapse` overlaps the cells on each grid line by that line's widest border, even where a cell's own border there is narrower, and a positioned cell moves the whole table's borders to where it paints; a row's, row group's, column's or column group's border competes on the cell sides that lie along it, and on the outer edges the table's own, each losing a tie to a cell's and to those before it, and the cells draw it where it wins, but not along a hidden cell or where no cell reaches the edge;
`justify` stretches lines to the widest line rather than the box (iced 0.14 lays justified text out at its widest line's width), leaves a line split at a float's end unstretched, and is start-aligned in rtl;
one line height and alignment per paragraph, the block's line height where its
inlines differ, and one wrapping too: `white-space: nowrap` or `pre` on part of a
paragraph does not keep that part on one line; text widens past
floats once per paragraph. Inline generated content takes `display`, `clear` and
text properties, and an inline one also its margins, padding, borders and background color, as an inline element's, a zero-width space holding them for `content:""`, with an inside list marker before that box, not in it; what it inherits moves with its element's animations, and its own color, background and border colors, visibility and font size, its padding, border widths and margins, and a block one's width and height too, animate when requested as `id::before` or `id::after` (on its line its background and its borders above and below fade to and from transparent as an inline element's, its borders across from a color to a color only, and its padding, border widths and margins across only where it has spaces for them at both ends and no negative margin pulls them; lengths it gives in `em`, and a line height it gives in `em` or a percentage, keep their pixels; its currentColor borders move with its color, other properties apply at once, its own `!important` is not told apart, and a list item's inside marker animates as its `::before`); generated content with any other `display` is a block that also takes `width`, `height`, margins, padding, borders and a background color, unless its element holds only text, where it joins that text. It takes no background images, positioning, floats, min and max sizes, `overflow`, and a `url()` in `content` shows nothing (its text items still show); `inherit` on its lengths and border widths takes its element's computed value. A press on it goes to the box beneath it, which for an inline element's generated block is that element's block container. `list-style-image` needs the `image` feature; inside the line, the image takes spaces as wide as it and half an em, which can break across lines and stretch under `justify`, centered on the line without making it taller, and a type's marker text stays in small capitals' lowercase. Inline box sides take spaces across, so widths round to about a sixth of the font size, a negative margin pulls its box over what is beside it as far as its own border and padding reach (its text keeps their remaining room, the glyphs beside it draw over its box, and its box draws over a box that follows it), and further only by narrowing the one space beside it, taken as a quarter of the font size, beside a percentage padding it takes its pull out of that padding alone, as wide as it is at layout (between inline-blocks and outside the widget, out of its fixed part), boxes around right-to-left words take it as approximately as their sides, a negative percentage margin is 0, and an animation to, from or across a negative margin, or of the border and padding beside one, moves its box as it pulls, while the space beside it stays as the target styling has it, a horizontal percentage is of the block's content width and applies only in the widget and not on the text between inline-blocks (in a shrink-to-fit box the text can then wrap), a vertical one likewise, taking no room in the line, a line can break between a side and its text, `justify` stretches them, right-to-left text keeps them in logical order, right-to-left text aligned `left` or `center` lays out a line at a time, spaces at their ends removed, and an inline element with no `:before` or `:after` whose content is only white space that collapses away, hidden elements, and inline elements empty in this way without sides, takes room only with padding, a border or a margin, and shows them (its text holds a zero-width space for it; an inner empty inline with sides holds the place itself, and the outer box wraps it), while other inlines without text, such as one holding only a `content:""` without sides or a float or positioned box, draw none; their height follows each run's font size. An rtl scroller scrolls across toward its left. `table-layout: fixed` reads the widths of `table-column` boxes directly in the table or in its column groups, not a group's own width, ignores column elements past the cells, keeps a right-anchored absolute table's left edge when its columns grow it, and lets content pick collapsed-border overlaps. A column's or column group's background color (not image) paints one rectangle from the first to the last cell that sits wholly inside it, the grid's full height, spacing included, so a column whose cells all span paints none, and a column group without columns counts as none.
A row of hidden empty cells takes no height, but a visible element inside a `visibility: hidden` cell there still draws, and content that draws nothing still extends a scroller's range. An own-text
scrolling list item's outside marker moves with the text, unclipped. A length to or from `auto` applies at once, as CSS has it without `interpolate-size`. A collapsed table's border color animates on the outer cells that draw it. Borders fade to and from transparent, a collapsed cell's except on a side that gives way along all of its edge at either end; a side with no width at either end, by width or by a `none` or `hidden` style, takes its color at once, a box's or an inline's, unless a width animates there or a `:hover`, `:active` or `:focus` rule could select the element, an ancestor or an earlier sibling of either, whatever it declares: one whose pseudo-class names their id or class, or names neither. Animations do not cover
font families and styles, or other keyword values; those and `!important` targets apply at once. An element that is inline at one end and a box at the other changes `display` at once, and its background and border colors move as the new one paints them. `font-weight` animates in steps of 100; a descendant whose weight is `bolder` or `lighter` than it, or otherwise not the same at both ends, takes its new weight at once. While `visibility` runs toward hidden, what the hidden styling settles in layout stays as it settles it, such as rows of hidden empty cells and collapsed borders of hidden cells. `collapse` animates as `hidden`. A `:before` or `:after` with a visibility of its own keeps it, as told by its content and order when requested. An inline element's background and border colors run from one color to another on its line, and to or from transparent as a layer over what shows beneath them, one inside another over the other's layer, on their block's background color, which is taken to be what lies beneath (not a table column's, nor what a positioned or floating box overlaps). With no such color (or a background image, or a block fading too), an inline without sides fading around a padded fading one shows the inner layer twice in the text, the outer between them. A descendant or border whose own color has the same two ends as the
animated `color` moves with it.

Not read at all: `letter-spacing` (iced has no way to set it), the paged-media properties, and URL cursors (their keyword fallback is used). A positive `word-spacing` adds stand-in spaces, rounding like inline sides, which stay at the end of a line a space breaks and keep a no-break space unbroken; a negative one draws each space smaller, a space taken as a quarter of the font size wide, down to none, so words never overlap. `text-indent` is approximated by leading spaces; a negative one hangs the first line out past the start edge (left, or right in a right-to-left block), laid out that much wider; a hanging first line stays beside floats as they are (the lines after it do not widen past a shorter one), a shrink-to-fit box sizes its text as without it, and `BoxTree::layout_text` reads none. A percentage is of the containing block's width, taken as the content width of the box the block sits in, and a shrink-to-fit box fits its spaces; it applies in the widget and `BoxTree::layout_with` (`BoxTree::layout_text` reads none). A word too wide beside it moves to the next line, and one wider than the block wraps into blank lines; a line of inline-blocks takes it as a whole, its first line starting that far in. `font-variant: small-caps` scales capitals, splitting combining marks from their letters and
sizing decorations by part; generated content and markers take it, `word-spacing`, `direction` and `unicode-bidi` only from their element.

## Cost

Measured on one machine in a release build (`tests/box_tree_scale.rs`): a cascade takes about 40 ms for 2,000 boxes and grows linearly; a layout with a fake shaper takes under 1 ms there. Each box tries only the rules whose selectors can match it by the id, class or type their last compound needs: 1,000 id rules over 2,000 boxes take about 70 ms (about 960 ms without that index on the same machine). Rules that need none of these, such as `*`, attribute-only or pseudo-class-only selectors, are still tried on every box. The index is not counted in the bundle's limits on retained bytes and work. The first hover of a box some `:hover` rule can select cascades the whole tree once and is then cached, up to 64 states: a class rule on every card means a stall per card on a large page.

A cascade over more than 100 million box and rule pairs, counting every rule whether the index would try it or not, is refused with an error; at that size simple selectors take about half a minute on the UI thread, and selectors that walk many ancestors take longer, since total matching work is not metered: treat stylesheets as trusted input, like code. A selector that overruns its own matching budget on an element it is tried on refuses the whole cascade (an element the index rules out does not try it): at construction that is an error, and for a later interaction the previous style stays.

## Coverage

`tests/property_coverage.rs` applies each of the 81 CSS 2.2 longhands in a page where it should matter and checks the result changes: 75 do. The rest are `letter-spacing` and the five paged-media properties. That shows a property is read, not that every value is right; there is no conformance suite.

## Status

Under active development and not yet published. Conformance is not measured.

## License

GPL-3.0-only; see `LICENSE`.
