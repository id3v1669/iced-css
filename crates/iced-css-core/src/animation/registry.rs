//! Atomic expansion and eligibility, before endpoint or engine mutation.
use crate::values::registry::{AnimationRule, PROPERTY_COUNT, PropertyId};
use std::num::NonZeroUsize;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SelectionError {
    Empty,
    Work,
    NotAnimatable(PropertyId),
    Undefined(PropertyId),
    InvalidRegistry(PropertyId),
}
impl std::fmt::Display for SelectionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "animation property selection: {self:?}")
    }
}
impl std::error::Error for SelectionError {}
#[derive(Debug, Clone)]
pub struct AnimationSelection {
    rules: [Option<AnimationRule>; PROPERTY_COUNT],
    count: usize,
}
impl AnimationSelection {
    /// Expand shorthands and reject the entire selection on any ineligible or
    /// undefined constituent. No endpoint, token or timing validation occurs here.
    pub fn new(properties: &[PropertyId], work: NonZeroUsize) -> Result<Self, SelectionError> {
        if properties.is_empty() {
            return Err(SelectionError::Empty);
        }
        let mut work = work.get();
        let mut selected = Self {
            rules: [None; PROPERTY_COUNT],
            count: 0,
        };
        for property in properties {
            work = work.checked_sub(1).ok_or(SelectionError::Work)?;
            for leaf in property.longhands() {
                work = work.checked_sub(1).ok_or(SelectionError::Work)?;
                let rule = leaf
                    .descriptor()
                    .animation
                    .ok_or(SelectionError::Undefined(*leaf))?;
                match rule {
                    AnimationRule::NotAnimatable => {
                        return Err(SelectionError::NotAnimatable(*leaf));
                    }
                    AnimationRule::Shorthand => return Err(SelectionError::InvalidRegistry(*leaf)),
                    _ => {}
                }
                if selected.rules[leaf.slot()].replace(rule).is_none() {
                    selected.count += 1;
                }
            }
        }
        Ok(selected)
    }
    pub fn len(&self) -> usize {
        self.count
    }
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }
    pub fn rule(&self, property: PropertyId) -> Option<AnimationRule> {
        self.rules[property.slot()]
    }
    /// Canonical registry order, independent of duplicate mentions in a request.
    pub fn properties(&self) -> impl Iterator<Item = PropertyId> + '_ {
        PropertyId::all().filter(|property| self.rules[property.slot()].is_some())
    }
}
