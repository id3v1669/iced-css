// Properties from CSS modules after CSS 2.2. Maintained by hand: append only, since a
// property's index is its identity and shorthands name their longhands by index.
// Definition text is reference metadata, not implemented grammar or computation.
use super::{AnimationRule, Inheritance, PropertyDescriptor, PropertyId, Scope};
/// Where this table starts: right after the CSS 2.2 table. Checked at compile time.
pub(super) const BASE: usize = 95;
pub(super) static PROPERTIES: &[PropertyDescriptor] = &[PropertyDescriptor {
    name: "box-sizing",
    source: "css-sizing-3#box-sizing",
    grammar: "content-box | border-box",
    initial: "content-box",
    applies_to: "all elements that accept width or height",
    percentages: "N/A",
    media: "visual",
    computed: "specified keyword",
    owner: "Box sizing and containing blocks",
    inheritance: Inheritance::No,
    scope: Scope::Screen,
    animation: Some(AnimationRule::Discrete),
    shorthand: false,
    targets: &[PropertyId(BASE)],
}];
