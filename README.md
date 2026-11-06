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
  quotes, `visibility`, outlines, `overflow` clipping and wheel scrolling.
- **Background images** (with the `image` Cargo feature): local files through `url()`, with `repeat`, `position` and
  `attachment`; relative paths resolve against `BoxTree::with_base`. Paths outside the base
  and remote URLs draw nothing.
- **Keyboard:** boxes with a press message or `tabindex` take focus by click and Tab, and
  match `:focus`; Enter and Space press them.
- **Native widgets:** any iced `Element` can sit in a leaf box and receives input.

## Known limits

Refused with an error rather than approximated: `inline-block` or `inline-table` mixed with
text in one line.

Approximated or missing: `border-collapse` shares each edge at the widest cell border, with
the later cell on top rather than resolving conflicts per edge; `justify` (start-aligned);
one line height and alignment per paragraph; inline box padding and borders; text widens past
floats once per paragraph; `::before` (CSS 2.2 uses `:before`). Generated content takes `display`, `clear` and
text properties, not other box properties. An rtl scroller does not scroll across.

## Status

Under active development and not yet published. Conformance is not measured.
