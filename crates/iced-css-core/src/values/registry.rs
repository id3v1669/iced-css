//! Property identities: the CSS 2.2 set, then the properties of later CSS modules.
//! Definition text is metadata, not an executable grammar.
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
pub(crate) const PROPERTY_COUNT: usize = PROPERTIES.len() + css3::PROPERTIES.len();
const _: () = assert!(css3::BASE == PROPERTIES.len());
fn table() -> impl ExactSizeIterator<Item = &'static PropertyDescriptor> + Clone {
    // Chained by hand: `Chain` is not an exact-size iterator.
    (0..PROPERTY_COUNT).map(|index| PropertyId(index).descriptor())
}
impl PropertyId {
    pub(crate) fn slot(self) -> usize {
        self.0
    }
    /// Look up a decoded property name; this does not tokenize or unescape CSS.
    pub fn lookup(name: &str) -> Option<Self> {
        table()
            .position(|p| p.name.eq_ignore_ascii_case(name))
            .map(Self)
    }
    pub fn all() -> impl ExactSizeIterator<Item = Self> {
        (0..PROPERTY_COUNT).map(Self)
    }
    /// The CSS 2.2 properties alone, which come first.
    pub fn css22() -> impl ExactSizeIterator<Item = Self> {
        (0..PROPERTIES.len()).map(Self)
    }
    /// Whether CSS 2.2 defines it; the rest come from later modules.
    pub fn is_css22(self) -> bool {
        self.0 < PROPERTIES.len()
    }
    pub fn descriptor(self) -> &'static PropertyDescriptor {
        match self.0.checked_sub(PROPERTIES.len()) {
            None => &PROPERTIES[self.0],
            Some(index) => &css3::PROPERTIES[index],
        }
    }
    /// Fully expanded identity set, including self for a longhand; no value expansion.
    pub fn longhands(self) -> &'static [Self] {
        self.descriptor().targets
    }
}
#[path = "registry_css3.rs"]
mod css3;
#[path = "registry_data.rs"]
mod data;
use data::PROPERTIES;
