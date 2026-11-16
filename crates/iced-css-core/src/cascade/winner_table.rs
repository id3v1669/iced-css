//! Atomic longhand/shorthand offers into an already selected target's cascade.
//! Source membership, media and selector admission are the caller's responsibility.
use super::{Candidate, CascadeKey, Origin, Specificity};
use crate::values::{
    property::{ParsedProperty, SpecifiedValue},
    registry::{PROPERTY_COUNT, PropertyId},
};
use std::{cmp::Ordering, num::NonZeroU64};
#[derive(Debug, Clone, Copy)]
pub struct TableAdmission {
    pub origin: Origin,
    pub specificity: Specificity,
    pub order: u64,
    pub declaration: NonZeroU64,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableError {
    InvalidExpansion,
    ConflictingOrder,
    ShorthandQuery,
}
impl std::fmt::Display for TableError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "property winner table: {self:?}")
    }
}
impl std::error::Error for TableError {}
/// Fixed-size storage borrowing immutable parsed leaves. Repeated identical keys
/// require the same declaration ID and the same leaf object, not a deep value
/// comparison. A recreated source generation needs a fresh evaluation table.
#[derive(Debug, Clone)]
pub struct PropertyWinners<'a> {
    slots: [Option<Candidate<&'a ParsedProperty>>; PROPERTY_COUNT],
}
impl Default for PropertyWinners<'_> {
    fn default() -> Self {
        Self {
            slots: std::array::from_fn(|_| None),
        }
    }
}
fn leaves(parsed: &ParsedProperty) -> Result<&[ParsedProperty], TableError> {
    if !parsed.property.descriptor().shorthand {
        return if matches!(parsed.value, SpecifiedValue::Expanded(_)) {
            Err(TableError::InvalidExpansion)
        } else {
            Ok(std::slice::from_ref(parsed))
        };
    }
    let SpecifiedValue::Expanded(leaves) = &parsed.value else {
        return Err(TableError::InvalidExpansion);
    };
    let targets = parsed.property.longhands();
    if leaves.len() != targets.len() {
        return Err(TableError::InvalidExpansion);
    }
    let mut seen = [false; PROPERTY_COUNT];
    for leaf in leaves {
        if leaf.property.descriptor().shorthand
            || matches!(leaf.value, SpecifiedValue::Expanded(_))
            || leaf.important != parsed.important
            || !targets.contains(&leaf.property)
            || seen[leaf.property.slot()]
        {
            return Err(TableError::InvalidExpansion);
        }
        seen[leaf.property.slot()] = true;
    }
    Ok(leaves)
}
impl<'a> PropertyWinners<'a> {
    pub fn new() -> Self {
        Self::default()
    }
    /// Borrow the winning specified value or select inheritance/an initial recipe.
    /// The returned borrow belongs to parsed source storage, not to this table.
    pub fn defaulted(
        &self,
        property: PropertyId,
        has_parent: bool,
    ) -> Result<super::DefaultedValueRef<'a>, super::DefaultingError> {
        if property.descriptor().shorthand {
            return Err(super::DefaultingError::Shorthand(property));
        }
        super::default_value_ref(
            property,
            self.slots[property.slot()]
                .as_ref()
                .map(|candidate| candidate.value),
            has_parent,
        )
    }
    pub fn candidate(
        &self,
        property: PropertyId,
    ) -> Result<Option<&Candidate<&'a ParsedProperty>>, TableError> {
        if property.descriptor().shorthand {
            return Err(TableError::ShorthandQuery);
        }
        Ok(self.slots[property.slot()].as_ref())
    }
    /// Put `value` in place of a longhand's winner, keeping its cascade position: for a
    /// winner that holds `var()`, once it is parsed for this element. `None` empties the
    /// slot: the value was invalid at computed-value time, and the property is inherited
    /// or initial. A slot with no winner stays empty.
    pub fn substitute(&mut self, property: PropertyId, value: Option<&'a ParsedProperty>) {
        let slot = &mut self.slots[property.slot()];
        match (slot.as_mut(), value) {
            (Some(candidate), Some(value)) => candidate.value = value,
            (Some(_), None) => *slot = None,
            (None, _) => {}
        }
    }
    /// CSS Animations 1, section 3: an animation's value for a longhand lies over every
    /// normal declaration and under the important ones. `value` is a longhand's leaf; a
    /// slot whose winner is important keeps it.
    pub fn animate(&mut self, value: &'a ParsedProperty) {
        let slot = &mut self.slots[value.property.slot()];
        if slot
            .as_ref()
            .is_some_and(|candidate| candidate.value.important)
        {
            return;
        }
        let key = slot.as_ref().map_or(
            CascadeKey {
                level: super::Level::AuthorNormal,
                specificity: Specificity::ZERO,
                order: u64::MAX,
            },
            |candidate| candidate.key,
        );
        *slot = Some(Candidate {
            key,
            declaration: NonZeroU64::MAX,
            value,
        });
    }
    /// The longhands whose winner still holds `var()`.
    pub fn pending(&self) -> impl Iterator<Item = (PropertyId, &'a ParsedProperty)> + '_ {
        self.slots.iter().filter_map(|slot| {
            let value = slot.as_ref()?.value;
            matches!(value.value, SpecifiedValue::Pending { .. }).then_some((value.property, value))
        })
    }
    /// Returns replaced slot count. Validate all expanded targets and order ties
    /// before changing any slot. Storage and structural loops have a registry-size
    /// bound and perform no allocation, data clone or arbitrary value comparison.
    pub fn offer(
        &mut self,
        parsed: &'a ParsedProperty,
        admission: TableAdmission,
    ) -> Result<usize, TableError> {
        let leaves = leaves(parsed)?;
        let key = CascadeKey {
            level: admission.origin.level(parsed.important),
            specificity: admission.specificity,
            order: admission.order,
        };
        let mut replace = [false; PROPERTY_COUNT];
        let mut count = 0;
        for leaf in leaves {
            let slot = leaf.property.slot();
            let comparison = self.slots[slot]
                .as_ref()
                .map_or(Ordering::Greater, |current| key.cmp(&current.key));
            match comparison {
                Ordering::Greater => {
                    replace[slot] = true;
                    count += 1;
                }
                Ordering::Less => {}
                Ordering::Equal => {
                    let current = self.slots[slot]
                        .as_ref()
                        .expect("an equal key requires a current candidate");
                    if current.declaration != admission.declaration
                        || !std::ptr::eq(current.value, leaf)
                    {
                        return Err(TableError::ConflictingOrder);
                    }
                }
            }
        }
        for leaf in leaves {
            if replace[leaf.property.slot()] {
                self.slots[leaf.property.slot()] = Some(Candidate {
                    key,
                    declaration: admission.declaration,
                    value: leaf,
                });
            }
        }
        Ok(count)
    }
}
