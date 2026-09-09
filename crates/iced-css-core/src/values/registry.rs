//! CSS 2.2 property identities. Definition text is metadata, not an executable grammar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PropertyId(usize);
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Scope {
    Screen,
    Paged,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inheritance {
    Yes,
    No,
    NoSeeProse,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnimationRule {
    ByComputedValue,
    Discrete,
    NotAnimatable,
    Shorthand,
    Visibility,
    FontWeight,
    FontStyle,
}
#[derive(Debug)]
pub struct PropertyDescriptor {
    pub name: &'static str,
    pub source: &'static str,
    pub grammar: &'static str,
    pub initial: &'static str,
    pub applies_to: &'static str,
    pub inheritance: Inheritance,
    pub percentages: &'static str,
    pub media: &'static str,
    pub computed: &'static str,
    pub scope: Scope,
    pub animation: Option<AnimationRule>,
    pub owner: &'static str,
    pub shorthand: bool,
    targets: &'static [PropertyId],
}
pub(crate) const PROPERTY_COUNT: usize = PROPERTIES.len();
impl PropertyId {
    pub(crate) fn slot(self) -> usize {
        self.0
    }
    /// Look up a decoded property name; this does not tokenize or unescape CSS.
    pub fn lookup(name: &str) -> Option<Self> {
        PROPERTIES
            .iter()
            .position(|p| p.name.eq_ignore_ascii_case(name))
            .map(Self)
    }
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        (0..PROPERTIES.len()).map(Self)
    }
    pub fn descriptor(self) -> &'static PropertyDescriptor {
        &PROPERTIES[self.0]
    }
    /// Fully expanded identity set, including self for a longhand; no value expansion.
    pub fn longhands(self) -> &'static [Self] {
        self.descriptor().targets
    }
}
#[path = "registry_data.rs"]
mod data;
use data::PROPERTIES;
