// Properties from CSS modules after CSS 2.2. Maintained by hand: append only, since a
// property's index is its identity and shorthands name their longhands by index.
// Definition text is reference metadata, not implemented grammar or computation.
use super::{AnimationRule, Inheritance, PropertyDescriptor, PropertyId, Scope};
/// Where this table starts: right after the CSS 2.2 table. Checked at compile time.
pub(super) const BASE: usize = 95;
pub(super) static PROPERTIES: &[PropertyDescriptor] = &[
    PropertyDescriptor {
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
    },
    // Not a property of its own: every custom property declaration (`--name: value`) is
    // carried under this one identity, as `SpecifiedValue::Custom`. Its slot in a winner table
    // holds whichever custom declaration won last and is never read; custom properties
    // cascade by name in `values::custom`.
    PropertyDescriptor {
        name: CUSTOM_PROPERTY,
        source: "css-variables-1#defining-variables",
        grammar: "<declaration-value>?",
        initial: "(guaranteed-invalid)",
        applies_to: "all elements",
        percentages: "N/A",
        media: "all",
        computed: "specified value with variables substituted",
        owner: "Cascade",
        inheritance: Inheritance::Yes,
        scope: Scope::Screen,
        animation: Some(AnimationRule::Discrete),
        shorthand: false,
        targets: &[PropertyId(BASE + 1)],
    },
];
/// The name the registry files every custom property under.
pub(super) const CUSTOM_PROPERTY: &str = "--*";
